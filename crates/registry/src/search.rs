use oatty_types::{CommandSpec, SchemaProperty, SearchResult};
use seekstorm::index::{
    AccessType, Document, DocumentCompression, FieldType, FrequentwordType, IndexArc, IndexDocuments, IndexMetaObject, NgramSet,
    QueryCompletion, SchemaField, SimilarityType, SpellingCorrection, StemmerType, StopwordType, Synonym, TokenizerType, create_index,
};
use seekstorm::search::{FacetFilter, QueryFacet, QueryRewriting, QueryType, ResultSort, ResultType, Search};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::{
    env,
    path::PathBuf,
    process,
    sync::{Arc, Mutex},
    thread,
};
use thiserror::Error;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

const CANONICAL_ID_FIELD: &str = "canonical_id";
const SUMMARY_FIELD: &str = "summary";
const SEARCH_CONTEXT_FIELD: &str = "search_context";
const INDEX_FIELD: &str = "index";

use crate::{CommandRegistry, models::CommandRegistryEvent};
struct CommandSearchEngine {
    command_registry: Arc<Mutex<CommandRegistry>>,
    index: Option<IndexArc>,
    fields: HashSet<String>,
}

impl CommandSearchEngine {
    pub fn new(command_registry: Arc<Mutex<CommandRegistry>>) -> Self {
        let fields: HashSet<String> =
            HashSet::from_iter([INDEX_FIELD.to_string(), CANONICAL_ID_FIELD.to_string(), SUMMARY_FIELD.to_string()]);
        CommandSearchEngine {
            command_registry,
            index: None,
            fields,
        }
    }
}

impl CommandSearchEngine {
    const DEFAULT_QUERY_OFFSET: usize = 0;
    const DEFAULT_RESULT_LENGTH: usize = 20;

    async fn handle_search_event(
        &self,
        term: String,
        index: &IndexArc,
        query_sender: &mpsc::Sender<Vec<SearchResult>>,
    ) -> Result<(), IndexerError> {
        let search_results = index
            .search(
                term,
                QueryType::Union,
                true,
                Self::DEFAULT_QUERY_OFFSET,
                Self::DEFAULT_RESULT_LENGTH,
                ResultType::Topk,
                true,
                Vec::new(),
                Vec::<QueryFacet>::new(),
                Vec::<FacetFilter>::new(),
                Vec::<ResultSort>::new(),
                QueryRewriting::SearchRewrite {
                    correct: Some(2),
                    distance: 1,
                    term_length_threshold: Some(vec![4]),
                    complete: Some(2),
                    length: Some(20),
                },
            )
            .await;

        let results = {
            let reader = index.read().await;
            let mut results: Vec<_> = Vec::with_capacity(search_results.results.len());

            for result in search_results.results {
                let doc = reader
                    .get_document(result.doc_id, true, &None, &self.fields, &Vec::new())
                    .await
                    .map_err(|e| IndexerError::Document(e.to_string()))?;

                results.push(SearchResult {
                    index: doc
                        .get(INDEX_FIELD)
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| IndexerError::Document("Index field missing or not u64".into()))?
                        as usize,

                    canonical_id: doc
                        .get(CANONICAL_ID_FIELD)
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| IndexerError::Document("Canonical ID missing or not string".into()))?
                        .to_string(),

                    summary: doc
                        .get(SUMMARY_FIELD)
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| IndexerError::Document("Summary missing or not string".into()))?
                        .to_string(),
                });
            }

            results
        };
        query_sender
            .send(results)
            .await
            .map_err(|error| IndexerError::Sender(error.to_string()))?;
        Ok(())
    }

    pub async fn start(
        &mut self,
        mut query_receiver: mpsc::Receiver<String>,
        query_sender: mpsc::Sender<Vec<SearchResult>>,
    ) -> Result<(), IndexerError> {
        if self.index.is_some() {
            return Err(IndexerError::Receiver("Indexer is already active".to_string()));
        };

        let index = self.index().await?;
        let registry = self.command_registry.clone();

        // initial index operation
        let new_documents = {
            let registry_guard = registry.lock().map_err(|e| IndexerError::Lock(e.to_string()))?;
            registry_guard
                .commands
                .iter()
                .enumerate()
                .map(|(i, command)| build_index_document(i, command))
                .collect()
        };
        index.index_documents(new_documents).await;

        let mut receiver = registry.lock().map_err(|e| IndexerError::Lock(e.to_string()))?.subscribe();
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    break;
                }
                event_result = receiver.recv() => {
                    self.handle_command_event(event_result).await?;
                }
                search_term = query_receiver.recv() => {
                    match search_term {
                        Some(term) => self.handle_search_event(term, &index, &query_sender).await?,
                        None => break,
                    }
                }
            }
        }

        Ok(())
    }

    async fn index_commands(&mut self, commands: Arc<[CommandSpec]>) -> Result<(), IndexerError> {
        let len = self
            .command_registry
            .lock()
            .map_err(|e| IndexerError::Lock(e.to_string()))?
            .commands
            .len();
        let index = self.index().await?;
        index
            .index_documents(
                commands
                    .iter()
                    .enumerate()
                    .map(|(i, command)| build_index_document(i + len, command))
                    .collect(),
            )
            .await;
        Ok(())
    }

    async fn handle_command_event(&mut self, event: Result<CommandRegistryEvent, RecvError>) -> Result<(), IndexerError> {
        match event {
            Ok(CommandRegistryEvent::CommandsAdded(commands)) => {
                if let Err(error) = self.index_commands(commands).await {
                    tracing::error!("Error updating index: {}", error);
                    return Err(error);
                }
                Ok(())
            }
            Err(error) => {
                tracing::error!("Error receiving event: {}", error);
                Err(IndexerError::Receiver(error.to_string()))
            }
            _ => Ok(()),
        }
    }

    async fn index(&mut self) -> Result<IndexArc, IndexerError> {
        if let Some(index) = self.index.as_ref() {
            return Ok(index.clone());
        }
        let index_path = Self::default_config_path();
        let access_type = AccessType::Ram;
        let meta = IndexMetaObject {
            id: 0,
            name: "tools".into(),
            similarity: SimilarityType::Bm25f,
            tokenizer: TokenizerType::UnicodeAlphanumeric,
            stemmer: StemmerType::English,
            stop_words: StopwordType::None,
            frequent_words: FrequentwordType::English,
            ngram_indexing: NgramSet::NgramFR as u8 | NgramSet::NgramRF as u8 | NgramSet::NgramFFR as u8,
            document_compression: DocumentCompression::None,
            access_type,
            spelling_correction: Some(SpellingCorrection {
                max_dictionary_edit_distance: 1,
                term_length_threshold: Some(vec![4]),
                count_threshold: 1,
                max_dictionary_entries: 10_000,
            }),
            query_completion: Some(QueryCompletion {
                max_completion_entries: 10_000,
            }),
        };

        let schema = schema();

        let index = create_index(&index_path, meta, &schema, &Vec::<Synonym>::new(), 11, false, None)
            .await
            .map_err(IndexerError::CreateIndex)?;
        self.index = Some(index.clone());

        Ok(index)
    }

    fn default_config_path() -> PathBuf {
        let process_id = process::id();
        env::temp_dir().join("oatty").join("tools").join(format!("in-memory-{process_id}"))
    }
}

fn schema() -> Vec<SchemaField> {
    vec![
        SchemaField::new(
            INDEX_FIELD.to_string(),
            true,  // stored
            false, // indexed
            FieldType::U64,
            false,
            false,
            1.0,
            true,
            false,
        ),
        SchemaField::new(
            CANONICAL_ID_FIELD.to_string(),
            true,
            true,
            FieldType::Text,
            false,
            false,
            1.0,
            true,
            true,
        ),
        SchemaField::new(
            SUMMARY_FIELD.to_string(),
            true,
            true,
            FieldType::Text,
            false,
            false,
            1.0,
            true,
            true,
        ),
        SchemaField::new(
            SEARCH_CONTEXT_FIELD.to_string(),
            false,
            true,
            FieldType::Text,
            false,
            false,
            1.2,
            false,
            false,
        ),
    ]
}

fn build_index_document(idx: usize, command: &CommandSpec) -> Document {
    let mut document_fields = Document::new();

    document_fields.insert(INDEX_FIELD.to_string(), json!(idx as u64));
    document_fields.insert(CANONICAL_ID_FIELD.to_string(), Value::String(command.canonical_id()));
    document_fields.insert(SUMMARY_FIELD.to_string(), Value::String(command.summary.to_owned()));
    document_fields.insert(SEARCH_CONTEXT_FIELD.to_string(), Value::String(build_search_context(command)));
    document_fields
}

fn build_search_context(command: &CommandSpec) -> String {
    let mut fragments = Vec::new();
    append_non_empty(&mut fragments, &command.canonical_id());
    append_non_empty(&mut fragments, &command.summary);
    append_optional(&mut fragments, normalize_identifier(&command.canonical_id()).as_deref());

    for positional_arg in &command.positional_args {
        append_non_empty(&mut fragments, &positional_arg.name);
        append_optional(&mut fragments, normalize_identifier(&positional_arg.name).as_deref());
        append_optional(&mut fragments, positional_arg.help.as_deref());
    }

    for flag in &command.flags {
        append_non_empty(&mut fragments, &flag.name);
        append_optional(&mut fragments, normalize_identifier(&flag.name).as_deref());
        append_optional(&mut fragments, flag.description.as_deref());
    }

    if let Some(http_command_spec) = command.http()
        && let Some(output_schema) = http_command_spec.output_schema.as_ref()
    {
        append_schema_descriptions(output_schema, &mut fragments);
    }

    fragments.join(" ")
}

fn append_schema_descriptions(property: &SchemaProperty, fragments: &mut Vec<String>) {
    append_non_empty(fragments, &property.description);

    if let Some(properties) = property.properties.as_ref() {
        let mut keys: Vec<&String> = properties.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(child_property) = properties.get(key) {
                append_schema_descriptions(child_property, fragments);
            }
        }
    }

    if let Some(items) = property.items.as_ref() {
        append_schema_descriptions(items, fragments);
    }
}

fn append_non_empty(fragments: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        fragments.push(trimmed.to_string());
    }
}

fn append_optional(fragments: &mut Vec<String>, value: Option<&str>) {
    if let Some(value) = value {
        append_non_empty(fragments, value);
    }
}

fn normalize_identifier(value: &str) -> Option<String> {
    let normalized = value.replace(['_', '-', '.'], " ");
    if normalized == value { None } else { Some(normalized) }
}

pub fn spawn_search_engine_thread(
    command_registry: Arc<Mutex<CommandRegistry>>,
) -> (mpsc::Receiver<Vec<SearchResult>>, mpsc::Sender<String>) {
    let (result_sender, result_receiver) = mpsc::channel::<Vec<SearchResult>>(100);
    let (query_sender, query_receiver) = mpsc::channel::<String>(100);

    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let local = tokio::task::LocalSet::new();
        let mut engine = CommandSearchEngine::new(command_registry);
        match rt.block_on(local.run_until(engine.start(query_receiver, result_sender))) {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Command search engine error: {}", e);
            }
        }
    });
    (result_receiver, query_sender)
}

#[derive(Debug, Error)]
pub enum IndexerError {
    #[error("Create index error: {0}")]
    CreateIndex(String),
    #[error("Update corpus error: {0}")]
    UpdateCorpus(String),
    #[error("Lock error: {0}")]
    Lock(String),
    #[error("Receiver error: {0}")]
    Receiver(String),
    #[error("Sender error: {0}")]
    Sender(String),
    #[error("Unknown error")]
    Document(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    use oatty_types::HttpCommandSpec;
    use tokio::time::{Duration, sleep};

    fn build_test_command_spec(group: &str, name: &str, summary: &str) -> CommandSpec {
        CommandSpec::new_http(
            group.to_owned(),
            name.to_owned(),
            summary.to_owned(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/test", None),
            0,
        )
    }

    fn unique_index_path() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time is before UNIX_EPOCH");
        env::temp_dir().join("oatty-tests").join(format!("search-index-{}", now.as_nanos()))
    }

    async fn wait_for_indexed_documents(index: &IndexArc, expected_count: usize) -> usize {
        for _ in 0..50 {
            let indexed = index.read().await.indexed_doc_count().await;
            if indexed >= expected_count {
                return indexed;
            }
            sleep(Duration::from_millis(10)).await;
        }
        index.read().await.indexed_doc_count().await
    }

    #[tokio::test]
    async fn search_rewrites_misspellings_in_small_corpus() {
        let index_path = unique_index_path();
        let access_type = AccessType::Ram;
        let meta = IndexMetaObject {
            id: 0,
            name: "test-search".into(),
            similarity: SimilarityType::Bm25f,
            tokenizer: TokenizerType::UnicodeAlphanumeric,
            stemmer: StemmerType::English,
            stop_words: StopwordType::None,
            frequent_words: FrequentwordType::English,
            ngram_indexing: NgramSet::NgramFR as u8 | NgramSet::NgramRF as u8 | NgramSet::NgramFFR as u8,
            document_compression: DocumentCompression::None,
            access_type,
            spelling_correction: Some(SpellingCorrection {
                max_dictionary_edit_distance: 1,
                term_length_threshold: Some(vec![4]),
                count_threshold: 1,
                max_dictionary_entries: 10_000,
            }),
            query_completion: Some(QueryCompletion {
                max_completion_entries: 10_000,
            }),
        };
        let index = create_index(&index_path, meta, &schema(), &Vec::<Synonym>::new(), 11, false, None)
            .await
            .expect("Failed to create test index");
        let command = build_test_command_spec("apps", "list", "List applications");
        let documents = vec![build_index_document(0, &command)];
        index.index_documents(documents).await;
        let indexed = wait_for_indexed_documents(&index, 1).await;
        assert_eq!(indexed, 1, "Expected one indexed document before searching");

        let original_query = "apps list".to_string();
        let search_results = index
            .search(
                original_query.clone(),
                QueryType::Intersection,
                true,
                0,
                5,
                ResultType::Topk,
                true,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                QueryRewriting::SearchRewrite {
                    correct: Some(2),
                    distance: 1,
                    term_length_threshold: Some(vec![4]),
                    complete: Some(2),
                    length: Some(5),
                },
            )
            .await;

        assert_eq!(search_results.result_count, 1, "Expected one result for query against test corpus");
    }
}
