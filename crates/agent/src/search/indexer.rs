use std::{collections::HashMap, env, mem, path::PathBuf, sync::{Arc, Mutex}};

use dirs_next::config_dir;
use oatty_mcp::{client::ClientGatewayEvent};
use oatty_registry::CommandRegistry;
use oatty_util::expand_tilde;
use serde_json::Value;
use tokio::{sync::broadcast::Receiver};
use seekstorm::{commit::{Commit}, index::{AccessType, FrequentwordType, IndexArc, IndexDocuments, IndexMetaObject, NgramSet, SchemaField, SimilarityType, StemmerType, StopwordType, Synonym, TokenizerType, create_index}};

pub struct Indexer {
    command_registry: Arc<Mutex<CommandRegistry>>,
    receiver: Option<Receiver<ClientGatewayEvent>>,
    index: Option<IndexArc>,
    listener_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Indexer {
    pub fn new(command_registry: Arc<Mutex<CommandRegistry>>, receiver:Receiver<ClientGatewayEvent>) -> Self {
        Indexer { command_registry, receiver: Some(receiver), index: None, listener_handle: None }
    }
}

impl Indexer {
    pub async fn spawn_listener(&mut self) -> Result<(), IndexerError> {
        let maybe_receiver = mem::take(&mut self.receiver);
        let Some(mut receiver) = maybe_receiver else {
            return Err(IndexerError::ReceiverError("Indexer is already active".to_string()));
        };
        
        let index = self.normalize_index().await?;
        let registry = self.command_registry.clone();
        
        let index_commands = async move || -> Result<(), IndexerError> {
            let new_documents = {
                let registry_lock = registry.lock().map_err(|e| IndexerError::LockError(e.to_string()))?;
                registry_lock.commands
                    .iter()
                    .map(|c| {
                        let mut hashmap = HashMap::new();
                        hashmap.insert("canonical_id".into(), Value::String(c.canonical_id()));
                        hashmap.insert("summary".into(), Value::String(c.summary.to_owned()));
                        hashmap
                    })
                    .collect()
            };
            
            index.index_documents(new_documents).await;
            index.commit().await;
            Ok(())
        };
        
        // initial index operation
        index_commands().await?;
        
        // auto update index when tools are updated
        let local = tokio::task::LocalSet::new();
        self.listener_handle = Some(local.run_until(async move {
            
            tokio::task::spawn_local(async move {
                loop {
                    if receiver.is_closed() {
                        break;
                    }
                    match receiver.recv().await {
                        Ok(ClientGatewayEvent::ToolsUpdated { .. }) => {
                            if let Err(e) = index_commands().await {
                                tracing::error!("Error updating index: {}", e);
                            }
                        },
                        Ok(_) => {},
                        Err(e) => tracing::error!("Error receiving event: {}", e),
                    }
                }
            })
            
        }).await);

        Ok(())
    }
    
    async fn normalize_index(&mut self) -> Result<IndexArc, IndexerError> {
        let index = if let Some(index) = &self.index {
            let mut index_lock = index.write().await;
            index_lock.clear_index().await;
            index.clone()
        } else {
            self.index().await?
        };
        Ok(index)
    }
    
    async fn index(&mut self) -> Result<IndexArc, IndexerError> {
        let index_path = Self::default_config_path();
        let meta = IndexMetaObject {
            id: 0,
            name: "tools".into(),
            similarity:SimilarityType::Bm25f,
            tokenizer:TokenizerType::UnicodeAlphanumeric,
            stemmer:StemmerType::English,
            stop_words: StopwordType::None,
            frequent_words:FrequentwordType::English,
            ngram_indexing: NgramSet::NgramFR as u8
                | NgramSet::NgramRF as u8
                | NgramSet::NgramFFR as u8,
            access_type: AccessType::Mmap,
            spelling_correction: None,
        };
        
        let schema = schema();
        let synonyms = synonyms();
        
        create_index(&index_path, meta, &schema, &synonyms, 11, false, None).await.map_err(|e| IndexerError::CreateIndexError(e))
    }
    
    fn default_config_path() -> PathBuf {
        if let Ok(path) = env::var("INDEX_PATH")
            && !path.trim().is_empty()
        {
            return expand_tilde(&path);
        }
    
        config_dir().unwrap_or_else(|| PathBuf::from(".")).join("heroku").join("tools")
    }
}

impl Drop for Indexer {
    fn drop(&mut self) {
        if let Some(listener_handle) = self.listener_handle.take() {
            listener_handle.abort();
        }
    }
}

fn schema() -> Vec<SchemaField> {
    let schema_json = r#"
    [{"field":"canonical_id","field_type":"Text","stored":true,"indexed":true},
    {"field":"summary","field_type":"Text","stored":true,"indexed":true}]"#;
    let schema: Vec<SchemaField> = serde_json::from_str(schema_json).unwrap();
    schema
}

fn synonyms() -> Vec<Synonym> {
    let synonyms_json = include_str!("./synonyms.json");
    let synonyms: Vec<Synonym> = serde_json::from_str(synonyms_json).unwrap();
    synonyms
}

#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    #[error("Create index error: {0}")]
    CreateIndexError(String),
    #[error("Update corpus error: {0}")]
    UpdateCorpusError(String),
    #[error("Lock error: {0}")]
    LockError(String),
    #[error("Receiver error: {0}")]
    ReceiverError(String),
}
