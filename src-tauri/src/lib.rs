use lopdf::{dictionary, Document, Object, ObjectId};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(serde::Serialize)]
pub struct PdfInfo {
    pub path: String,
    pub name: String,
    pub pages: u32,
}

#[tauri::command]
fn get_pdf_info(path: String) -> Result<PdfInfo, String> {
    let doc = Document::load(&path).map_err(|e| format!("Failed to load PDF: {}", e))?;
    let pages = doc.get_pages().len() as u32;
    let name = Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.pdf")
        .to_string();

    Ok(PdfInfo { path, name, pages })
}

#[tauri::command]
fn combine_pdfs(paths: Vec<String>, output_path: String) -> Result<String, String> {
    if paths.is_empty() {
        return Err("No PDF files provided".to_string());
    }

    // Load all documents
    let documents: Vec<Document> = paths
        .iter()
        .map(|path| Document::load(path).map_err(|e| format!("Failed to load {}: {}", path, e)))
        .collect::<Result<Vec<_>, _>>()?;

    // Start with an empty document
    let mut combined_doc = Document::with_version("1.5");
    let mut combined_pages: Vec<ObjectId> = Vec::new();

    // Track the max object ID we've used
    let mut max_id: u32 = 1;

    for doc in documents {
        // Create a mapping from old object IDs to new object IDs
        let mut id_mapping: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();

        // First pass: assign new IDs to all objects
        for (old_id, _) in doc.objects.iter() {
            let new_id = (max_id, 0);
            id_mapping.insert(*old_id, new_id);
            max_id += 1;
        }

        // Second pass: copy objects with updated references
        for (old_id, object) in doc.objects.iter() {
            let new_id = id_mapping[old_id];
            let new_object = remap_object_refs(object.clone(), &id_mapping);
            combined_doc.objects.insert(new_id, new_object);
        }

        // Collect page object IDs (remapped)
        let pages = doc.get_pages();
        for (_, &page_id) in pages.iter() {
            if let Some(&new_page_id) = id_mapping.get(&page_id) {
                combined_pages.push(new_page_id);
            }
        }
    }

    // Build the page tree
    let pages_id = (max_id, 0);
    max_id += 1;

    // Update each page to point to the new parent
    for &page_id in &combined_pages {
        if let Some(Object::Dictionary(ref mut dict)) = combined_doc.objects.get_mut(&page_id) {
            dict.set("Parent", Object::Reference(pages_id));
        }
    }

    // Create the Pages object
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Count" => Object::Integer(combined_pages.len() as i64),
        "Kids" => combined_pages.iter().map(|&id| Object::Reference(id)).collect::<Vec<_>>(),
    };
    combined_doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Create the Catalog
    let catalog_id = (max_id, 0);
    let catalog_dict = dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    };
    combined_doc.objects.insert(catalog_id, Object::Dictionary(catalog_dict));

    // Set the trailer
    combined_doc.trailer.set("Root", Object::Reference(catalog_id));

    // Compress and save
    combined_doc.compress();
    combined_doc
        .save(&output_path)
        .map_err(|e| format!("Failed to save PDF: {}", e))?;

    Ok(output_path)
}

fn remap_object_refs(object: Object, mapping: &BTreeMap<ObjectId, ObjectId>) -> Object {
    match object {
        Object::Reference(id) => {
            Object::Reference(*mapping.get(&id).unwrap_or(&id))
        }
        Object::Array(arr) => {
            Object::Array(arr.into_iter().map(|o| remap_object_refs(o, mapping)).collect())
        }
        Object::Dictionary(dict) => {
            let mut new_dict = lopdf::Dictionary::new();
            for (key, value) in dict.into_iter() {
                new_dict.set(key, remap_object_refs(value, mapping));
            }
            Object::Dictionary(new_dict)
        }
        Object::Stream(mut stream) => {
            let mut new_dict = lopdf::Dictionary::new();
            for (key, value) in stream.dict.into_iter() {
                new_dict.set(key, remap_object_refs(value, mapping));
            }
            stream.dict = new_dict;
            Object::Stream(stream)
        }
        _ => object,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![get_pdf_info, combine_pdfs])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
