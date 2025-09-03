//! OpenAPI document transformation utilities.
//! 
//! This module provides functionality to transform OpenAPI v2 (Swagger) and v3 documents
//! into a minimal hyper-schema-like format with a `links` array for command generation.

use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

// ============================================================================
// Utility Functions
// ============================================================================

/// Converts HTTP method to uppercase for consistency.
fn upper_http(method: &str) -> String {
    method.to_ascii_uppercase()
}

/// Determines if a document is OpenAPI v3 based on the presence of an "openapi" field.
fn is_oas3(doc: &Value) -> bool {
    doc.get("openapi").and_then(Value::as_str).is_some()
}

/// Escapes JSON pointer tokens according to RFC 6901.
/// 
/// Replaces `~` with `~0` and `/` with `~1` for safe JSON pointer usage.
fn escape_json_pointer_token(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// Resolves local JSON references within the same document.
/// 
/// Handles references like "#/components/schemas/Foo" or "#/components/parameters/Bar"
/// by stripping the '#' prefix and using JSON pointer resolution.
fn resolve_local_ref<'a>(root: &'a Value, r: &str) -> Option<Value> {
    let ptr = r.strip_prefix('#').unwrap_or(r);
    root.pointer(ptr).cloned()
}

// ============================================================================
// Parameter Collection and Merging
// ============================================================================

/// Collects and merges parameters from path items and operations.
/// 
/// Operation-level parameters override path-level parameters when they have
/// the same name and location (in). Returns a deduplicated list of parameters.
fn collect_parameters(root: &Value, path_item: &Value, op: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut seen: Vec<(String, String)> = Vec::new();

    let push_param = |out: &mut Vec<Value>, seen: &mut Vec<(String, String)>, p: Value| {
        let (name, location) = (
            p.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            p.get("in").and_then(Value::as_str).unwrap_or("").to_string(),
        );
        
        if !name.is_empty() && !location.is_empty() {
            // Replace existing parameter with same name+location
            if let Some(idx) = seen.iter().position(|(n, i)| n == &name && i == &location) {
                out[idx] = p;
            } else {
                out.push(p);
                seen.push((name, location));
            }
        }
    };

    // Process path-level parameters
    if let Some(arr) = path_item.get("parameters").and_then(Value::as_array) {
        for p in arr {
            let resolved = if let Some(r) = p.get("$ref").and_then(Value::as_str) {
                resolve_local_ref(root, r).unwrap_or_else(|| p.clone())
            } else {
                p.clone()
            };
            push_param(&mut out, &mut seen, resolved);
        }
    }

    // Process operation-level parameters (these override path-level ones)
    if let Some(arr) = op.get("parameters").and_then(Value::as_array) {
        for p in arr {
            let resolved = if let Some(r) = p.get("$ref").and_then(Value::as_str) {
                resolve_local_ref(root, r).unwrap_or_else(|| p.clone())
            } else {
                p.clone()
            };
            push_param(&mut out, &mut seen, resolved);
        }
    }

    out
}

/// Merges required field arrays, avoiding duplicates.
fn merge_required(into: &mut Vec<String>, from: Option<&Value>) {
    if let Some(arr) = from.and_then(Value::as_array) {
        for n in arr.iter().filter_map(|v| v.as_str()) {
            if !into.contains(&n.to_string()) {
                into.push(n.to_string());
            }
        }
    }
}

/// Merges property maps, preserving existing values.
fn merge_properties(into: &mut Map<String, Value>, from: Option<&Map<String, Value>>) {
    if let Some(map) = from {
        for (k, v) in map.iter() {
            into.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
}

// ============================================================================
// Schema Building
// ============================================================================

/// Builds a link schema from OpenAPI v3 operation parameters and request body.
/// 
/// Converts query parameters to flags and request body to properties,
/// creating a unified schema for the command.
fn build_link_schema_from_oas3(root: &Value, path_item: &Value, op: &Value) -> Option<Value> {
    let params = collect_parameters(root, path_item, op);
    let mut required: Vec<String> = Vec::new();
    let mut props: Map<String, Value> = Map::new();

    // Convert query parameters to flags
    for p in params {
        let location = p.get("in").and_then(Value::as_str).unwrap_or("");
        if location == "query" {
            let name = match p.get("name").and_then(Value::as_str) {
                Some(n) => n.to_string(),
                None => continue,
            };

            // Mark as required if specified
            if p.get("required").and_then(Value::as_bool) == Some(true) {
                if !required.contains(&name) {
                    required.push(name.clone());
                }
            }

            // Build parameter schema
            let mut schema = p.get("schema").cloned().unwrap_or_else(|| json!({}));
            
            // Promote description and default from parameter to schema
            if let Some(desc) = p.get("description").cloned() {
                if schema.get("description").is_none() {
                    if let Some(obj) = schema.as_object_mut() {
                        obj.insert("description".into(), desc);
                    }
                }
            }
            
            if let Some(def) = p.get("default").cloned() {
                if schema.get("default").is_none() {
                    if let Some(obj) = schema.as_object_mut() {
                        obj.insert("default".into(), def);
                    }
                }
            }
            
            props.insert(name, schema);
        }
    }

    // Handle request body schema
    if let Some(rb_schema) = op
        .get("requestBody")
        .and_then(|rb| rb.get("content"))
        .and_then(|c| c.get("application/json"))
        .and_then(|aj| aj.get("schema"))
    {
        let body_schema = if let Some(r) = rb_schema.get("$ref").and_then(Value::as_str) {
            resolve_local_ref(root, r).unwrap_or_else(|| rb_schema.clone())
        } else {
            rb_schema.clone()
        };

        if let Some(obj) = body_schema.as_object() {
            if obj.get("properties").is_some() {
                // Merge object properties
                let body_props = obj
                    .get("properties")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                merge_properties(&mut props, Some(&body_props));
                merge_required(&mut required, obj.get("required"));
            } else {
                // Fallback: expose as synthetic "body" property
                props.entry("body").or_insert_with(|| body_schema);
            }
        } else {
            props.entry("body").or_insert_with(|| body_schema);
        }
    }

    // Return schema only if we have properties or required fields
    if props.is_empty() && required.is_empty() {
        None
    } else {
        let mut schema = Map::new();
        schema.insert("type".into(), json!("object"));
        schema.insert("properties".into(), Value::Object(props));
        if !required.is_empty() {
            schema.insert("required".into(), json!(required));
        }
        Some(Value::Object(schema))
    }
}

// ============================================================================
// HREF Rewriting
// ============================================================================

/// Rewrites HREF to embed JSON pointers for path parameters.
/// 
/// Converts path segments like `{id}` to `{(#/x-parameters/paths/.../id)}`
/// to enable dynamic parameter resolution.
fn rewrite_href_with_param_pointers(path: &str, params: &Vec<Value>) -> String {
    // Build name->schema map for path parameters
    let mut param_map: HashMap<String, Value> = HashMap::new();
    for p in params {
        if p.get("in").and_then(Value::as_str) == Some("path") {
            if let Some(name) = p.get("name").and_then(Value::as_str) {
                param_map.insert(name.to_string(), p.get("schema").cloned().unwrap_or_else(|| p.clone()));
            }
        }
    }

    if param_map.is_empty() {
        return path.to_string();
    }

    let escaped_path = escape_json_pointer_token(path);
    let mut segments = Vec::new();

    for segment in path.split('/') {
        if segment.starts_with('{') && segment.ends_with('}') {
            let param_name = segment.trim_start_matches('{').trim_end_matches('}');
            if param_map.contains_key(param_name) {
                let escaped_name = escape_json_pointer_token(param_name);
                let pointer = format!("#/x-parameters/paths/{}/{}", escaped_path, escaped_name);
                let enc = utf8_percent_encode(&pointer, NON_ALPHANUMERIC).to_string();
                segments.push(format!("{{({})}}", enc));
            } else {
                segments.push(segment.to_string());
            }
        } else {
            segments.push(segment.to_string());
        }
    }

    segments.join("/")
}

// ============================================================================
// REL Field Determination
// ============================================================================

/// Determines the "rel" field for a link based on HTTP method and path pattern.
/// 
/// Uses common hyper-schema conventions:
/// - GET /resources (no path params) -> "instances"
/// - POST /resources -> "create" 
/// - GET /resources/{id} -> "self"
/// - PATCH/PUT /resources/{id} -> "update"
/// - DELETE /resources/{id} -> "delete"
fn determine_rel(method: &str, path: &str) -> String {
    let has_path_params = path.contains('{') && path.contains('}');
    let is_collection = !has_path_params;
    
    match (method.to_lowercase().as_str(), is_collection) {
        ("get", true) => "instances".to_string(),
        ("post", _) => "create".to_string(),
        ("get", false) => "self".to_string(),
        ("put" | "patch", false) => "update".to_string(),
        ("delete", false) => "delete".to_string(),
        _ => {
            // Fallback: use method + path pattern
            if is_collection {
                format!("{}_{}", method.to_lowercase(), "collection")
            } else {
                format!("{}_{}", method.to_lowercase(), "resource")
            }
        }
    }
}

// ============================================================================
// Main Transformation Functions
// ============================================================================

/// Transforms an OpenAPI document into a minimal hyper-schema-like format.
/// 
/// Supports both OpenAPI v3 and Swagger v2 documents, converting them to a
/// unified format with a `links` array and preserved components for reference resolution.
/// 
/// # Arguments
/// * `doc` - The OpenAPI document as a JSON Value
/// 
/// # Returns
/// * `Result<Value>` - Transformed document or error
/// 
/// # Errors
/// * Returns error for unsupported document types
pub fn transform_openapi_to_links(doc: &Value) -> Result<Value> {
    if is_oas3(doc) {
        transform_oas3(doc)
    } else if doc.get("swagger").is_some() {
        transform_swagger2(doc)
    } else {
        Err(anyhow!("Unsupported OpenAPI document: expected v3 (openapi) or v2 (swagger)"))
    }
}

/// Transforms OpenAPI v3 documents to the target format.
fn transform_oas3(doc: &Value) -> Result<Value> {
    let mut links: Vec<Value> = Vec::new();
    let mut x_params: HashMap<String, Map<String, Value>> = HashMap::new();

    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("OpenAPI document missing paths"))?;

    // Process each path and operation
    for (path, path_item) in paths.iter() {
        let path_obj = path_item
            .as_object()
            .ok_or_else(|| anyhow!("Path item not an object: {}", path))?;

        for (method, operation) in path_obj.iter() {
            match method.as_str() {
                "get" | "post" | "put" | "patch" | "delete" => {
                    let link = build_link_from_operation(doc, path_item, operation, method, path)?;
                    
                    // Collect path parameters for help system
                    let params = collect_parameters(doc, path_item, operation);
                    let path_params = collect_path_parameters(&params);
                    if !path_params.is_empty() {
                        x_params.insert(path.clone(), path_params);
                    }
                    
                    links.push(link);
                }
                _ => {} // Skip non-HTTP methods
            }
        }
    }

    // Build final document
    let mut root = Map::new();
    root.insert("links".into(), Value::Array(links));
    
    // Preserve components for reference resolution
    if let Some(components) = doc.get("components").cloned() {
        root.insert("components".into(), components);
    }
    
    // Add x-parameters for path parameter help
    if !x_params.is_empty() {
        let mut xp = Map::new();
        for (path, params) in x_params {
            xp.insert(path, Value::Object(params));
        }
        let mut xroot = Map::new();
        xroot.insert("paths".into(), Value::Object(xp));
        root.insert("x-parameters".into(), Value::Object(xroot));
    }
    
    Ok(Value::Object(root))
}

/// Transforms Swagger v2 documents to the target format.
fn transform_swagger2(doc: &Value) -> Result<Value> {
    let mut links: Vec<Value> = Vec::new();
    let mut x_params: HashMap<String, Map<String, Value>> = HashMap::new();

    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("Swagger v2 document missing paths"))?;

    // Process each path and operation
    for (path, path_item) in paths.iter() {
        let path_obj = path_item
            .as_object()
            .ok_or_else(|| anyhow!("Path item not an object: {}", path))?;

        for (method, operation) in path_obj.iter() {
            match method.as_str() {
                "get" | "post" | "put" | "patch" | "delete" => {
                    let link = build_link_from_swagger2_operation(doc, path_item, operation, method, path)?;
                    
                    // Collect path parameters for help system
                    let all_params = collect_swagger2_parameters(doc, path_item, operation);
                    let path_params = collect_path_parameters(&all_params);
                    if !path_params.is_empty() {
                        x_params.insert(path.clone(), path_params);
                    }
                    
                    links.push(link);
                }
                _ => {} // Skip non-HTTP methods
            }
        }
    }

    // Build final document
    let mut root = Map::new();
    root.insert("links".into(), Value::Array(links));
    
    // Preserve definitions and parameters for reference resolution
    if let Some(defs) = doc.get("definitions").cloned() {
        root.insert("definitions".into(), defs);
    }
    if let Some(params) = doc.get("parameters").cloned() {
        root.insert("parameters".into(), params);
    }
    
    // Add x-parameters for path parameter help
    if !x_params.is_empty() {
        let mut xp = Map::new();
        for (path, params) in x_params {
            xp.insert(path, Value::Object(params));
        }
        let mut xroot = Map::new();
        xroot.insert("paths".into(), Value::Object(xp));
        root.insert("x-parameters".into(), Value::Object(xroot));
    }
    
    Ok(Value::Object(root))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Builds a link object from an OpenAPI v3 operation.
fn build_link_from_operation(
    doc: &Value,
    path_item: &Value,
    op: &Value,
    method: &str,
    path: &str,
) -> Result<Value> {
    let title = op
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| op.get("operationId").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    
    let description = op
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or(&title)
        .to_string();

    let params = collect_parameters(doc, path_item, op);
    let href = rewrite_href_with_param_pointers(path, &params);
    let schema = build_link_schema_from_oas3(doc, path_item, op);

    let mut link_obj = Map::new();
    link_obj.insert("href".into(), json!(href));
    link_obj.insert("method".into(), json!(upper_http(method)));
    link_obj.insert("rel".into(), json!(determine_rel(method, path)));
    
    if !title.is_empty() {
        link_obj.insert("title".into(), json!(title));
    }
    if !description.is_empty() {
        link_obj.insert("description".into(), json!(description));
    }
    if let Some(s) = schema {
        link_obj.insert("schema".into(), s);
    }
    
    Ok(Value::Object(link_obj))
}

/// Builds a link object from a Swagger v2 operation.
fn build_link_from_swagger2_operation(
    doc: &Value,
    path_item: &Value,
    op: &Value,
    method: &str,
    path: &str,
) -> Result<Value> {
    let title = op
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| op.get("operationId").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    
    let description = op
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or(&title)
        .to_string();

    let all_params = collect_swagger2_parameters(doc, path_item, op);
    let href = rewrite_href_with_param_pointers(path, &all_params);
    let schema = build_swagger2_link_schema(doc, &all_params);

    let mut link_obj = Map::new();
    link_obj.insert("href".into(), json!(href));
    link_obj.insert("method".into(), json!(upper_http(method)));
    link_obj.insert("rel".into(), json!(determine_rel(method, path)));
    
    if !title.is_empty() {
        link_obj.insert("title".into(), json!(title));
    }
    if !description.is_empty() {
        link_obj.insert("description".into(), json!(description));
    }
    if let Some(s) = schema {
        link_obj.insert("schema".into(), s);
    }
    
    Ok(Value::Object(link_obj))
}

/// Collects all parameters for a Swagger v2 operation, resolving references.
fn collect_swagger2_parameters(doc: &Value, path_item: &Value, op: &Value) -> Vec<Value> {
    let mut all_params: Vec<Value> = Vec::new();
    
    // Add path-level parameters
    if let Some(arr) = path_item.get("parameters").and_then(Value::as_array) {
        for p in arr {
            all_params.push(resolve_swagger2_param_ref(doc, p));
        }
    }
    
    // Add operation-level parameters (these override path-level ones)
    if let Some(arr) = op.get("parameters").and_then(Value::as_array) {
        for p in arr {
            all_params.push(resolve_swagger2_param_ref(doc, p));
        }
    }
    
    all_params
}

/// Resolves a Swagger v2 parameter reference.
fn resolve_swagger2_param_ref(root: &Value, p: &Value) -> Value {
    if let Some(r) = p.get("$ref").and_then(Value::as_str) {
        resolve_local_ref(root, r).unwrap_or_else(|| p.clone())
    } else {
        p.clone()
    }
}

/// Collects path parameters from a parameter list.
fn collect_path_parameters(params: &[Value]) -> Map<String, Value> {
    let mut path_params: Map<String, Value> = Map::new();
    
    for p in params {
        if p.get("in").and_then(Value::as_str) == Some("path") {
            if let Some(name) = p.get("name").and_then(Value::as_str) {
                let schema = p.get("schema").cloned().unwrap_or_else(|| p.clone());
                path_params.insert(name.to_string(), schema);
            }
        }
    }
    
    path_params
}

/// Builds a link schema from Swagger v2 parameters.
fn build_swagger2_link_schema(root: &Value, params: &[Value]) -> Option<Value> {
    let mut required: Vec<String> = Vec::new();
    let mut properties: Map<String, Value> = Map::new();

    for param in params {
        match param.get("in").and_then(Value::as_str) {
            Some("query") => {
                if let Some(name) = param.get("name").and_then(Value::as_str) {
                    // Mark as required if specified
                    if param.get("required").and_then(Value::as_bool) == Some(true) {
                        if !required.contains(&name.to_string()) {
                            required.push(name.to_string());
                        }
                    }
                    
                    // Build parameter schema
                    let mut schema = param.get("schema").cloned().unwrap_or_else(|| json!({}));
                    
                    // Handle Swagger v2 parameter format (type/default at top-level)
                    if schema.is_null() || !schema.is_object() {
                        let mut s = Map::new();
                        if let Some(t) = param.get("type").cloned() {
                            s.insert("type".into(), t);
                        }
                        if let Some(e) = param.get("enum").cloned() {
                            s.insert("enum".into(), e);
                        }
                        if let Some(d) = param.get("default").cloned() {
                            s.insert("default".into(), d);
                        }
                        if let Some(desc) = param.get("description").cloned() {
                            s.insert("description".into(), desc);
                        }
                        schema = Value::Object(s);
                    } else {
                        // Ensure description and default are present
                        if schema.get("description").is_none() {
                            if let Some(desc) = param.get("description").cloned() {
                                if let Some(obj) = schema.as_object_mut() {
                                    obj.insert("description".into(), desc);
                                }
                            }
                        }
                        if schema.get("default").is_none() {
                            if let Some(def) = param.get("default").cloned() {
                                if let Some(obj) = schema.as_object_mut() {
                                    obj.insert("default".into(), def);
                                }
                            }
                        }
                    }
                    
                    properties.insert(name.to_string(), schema);
                }
            }
            Some("body") => {
                if let Some(body_schema) = param.get("schema") {
                    let schema = if let Some(r) = body_schema.get("$ref").and_then(Value::as_str) {
                        resolve_local_ref(root, r).unwrap_or_else(|| body_schema.clone())
                    } else {
                        body_schema.clone()
                    };
                    
                    if let Some(obj) = schema.as_object() {
                        if obj.get("properties").is_some() {
                            let body_props = obj
                                .get("properties")
                                .and_then(Value::as_object)
                                .cloned()
                                .unwrap_or_default();
                            merge_properties(&mut properties, Some(&body_props));
                            merge_required(&mut required, obj.get("required"));
                        } else {
                            properties.entry("body").or_insert_with(|| schema.clone());
                        }
                    } else {
                        properties.entry("body").or_insert_with(|| schema);
                    }
                }
            }
            _ => {} // Skip other parameter types
        }
    }

    // Return schema only if we have properties or required fields
    if properties.is_empty() && required.is_empty() {
        None
    } else {
        let mut schema = Map::new();
        schema.insert("type".into(), json!("object"));
        schema.insert("properties".into(), Value::Object(properties));
        if !required.is_empty() {
            schema.insert("required".into(), json!(required));
        }
        Some(Value::Object(schema))
    }
}
