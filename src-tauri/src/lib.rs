use lopdf::{Document, Object, ObjectId};
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

    // Starting max_id for renumbering
    let mut max_id = 1;

    // Collect all pages and objects
    let mut documents_pages: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut documents_objects: BTreeMap<ObjectId, Object> = BTreeMap::new();

    // The output document
    let mut document = Document::with_version("1.5");

    for mut doc in documents {
        // Renumber objects to avoid ID conflicts
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        // Collect pages
        let pages = doc.get_pages();
        for object_id in pages.into_values() {
            if let Ok(object) = doc.get_object(object_id) {
                documents_pages.insert(object_id, object.clone());
            }
        }

        // Collect all objects
        documents_objects.extend(doc.objects);
    }

    // Find Catalog and Pages objects, insert other objects
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in documents_objects.into_iter() {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                if catalog_object.is_none() {
                    catalog_object = Some((object_id, object));
                }
            }
            b"Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref existing)) = pages_object {
                        if let Ok(old_dict) = existing.as_dict() {
                            dictionary.extend(old_dict);
                        }
                    }
                    pages_object = Some((
                        pages_object.as_ref().map(|(id, _)| *id).unwrap_or(object_id),
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            b"Page" => {} // Handled separately
            b"Outlines" | b"Outline" => {} // Skip outlines
            _ => {
                document.objects.insert(object_id, object);
            }
        }
    }

    let (pages_id, pages_obj) = pages_object.ok_or("No Pages object found in PDFs")?;
    let (catalog_id, catalog_obj) = catalog_object.ok_or("No Catalog object found in PDFs")?;

    // Insert page objects with updated parent reference
    for (object_id, object) in documents_pages.iter() {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_id);
            document.objects.insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    // Build new Pages object
    if let Ok(dictionary) = pages_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", documents_pages.len() as u32);
        dictionary.set(
            "Kids",
            documents_pages
                .keys()
                .map(|&id| Object::Reference(id))
                .collect::<Vec<_>>(),
        );
        document.objects.insert(pages_id, Object::Dictionary(dictionary));
    }

    // Build new Catalog object
    if let Ok(dictionary) = catalog_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_id);
        dictionary.remove(b"Outlines");
        document.objects.insert(catalog_id, Object::Dictionary(dictionary));
    }

    // Set trailer
    document.trailer.set("Root", catalog_id);
    document.max_id = document.objects.len() as u32;

    // Renumber and save
    document.renumber_objects();
    document
        .save(&output_path)
        .map_err(|e| format!("Failed to save PDF: {}", e))?;

    Ok(output_path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn get_examples_dir() -> std::path::PathBuf {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        Path::new(manifest_dir).parent().unwrap().join("examples")
    }

    #[test]
    fn test_get_pdf_info_jabberwocky() {
        let path = get_examples_dir().join("jabberwocky.pdf");
        let result = get_pdf_info(path.to_string_lossy().to_string());

        assert!(result.is_ok(), "Failed to get PDF info: {:?}", result.err());
        let info = result.unwrap();
        assert_eq!(info.name, "jabberwocky.pdf");
        assert!(info.pages >= 1, "Expected at least 1 page");
    }

    #[test]
    fn test_get_pdf_info_shakespeare() {
        let path = get_examples_dir().join("shakespeare_sonnets.pdf");
        let result = get_pdf_info(path.to_string_lossy().to_string());

        assert!(result.is_ok(), "Failed to get PDF info: {:?}", result.err());
        let info = result.unwrap();
        assert_eq!(info.name, "shakespeare_sonnets.pdf");
        assert!(info.pages >= 1, "Expected at least 1 page");
    }

    #[test]
    fn test_get_pdf_info_nonexistent() {
        let result = get_pdf_info("/nonexistent/path.pdf".to_string());
        assert!(result.is_err(), "Should fail for nonexistent file");
    }

    #[test]
    fn test_combine_pdfs_empty_list() {
        let result = combine_pdfs(vec![], "/tmp/output.pdf".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No PDF files provided");
    }

    #[test]
    fn test_combine_pdfs_single_file() {
        let examples = get_examples_dir();
        let input = examples.join("jabberwocky.pdf");
        let output = std::env::temp_dir().join("test_single.pdf");

        let result = combine_pdfs(
            vec![input.to_string_lossy().to_string()],
            output.to_string_lossy().to_string(),
        );

        assert!(result.is_ok(), "Failed to combine single PDF: {:?}", result.err());
        assert!(output.exists(), "Output file should exist");

        // Verify output is valid PDF
        let doc = Document::load(&output);
        assert!(doc.is_ok(), "Output should be valid PDF: {:?}", doc.err());

        // Cleanup
        let _ = fs::remove_file(&output);
    }

    #[test]
    fn test_combine_pdfs_two_files() {
        let examples = get_examples_dir();
        let input1 = examples.join("jabberwocky.pdf");
        let input2 = examples.join("shakespeare_sonnets.pdf");
        let output = std::env::temp_dir().join("test_combined.pdf");

        // Get page counts
        let info1 = get_pdf_info(input1.to_string_lossy().to_string()).unwrap();
        let info2 = get_pdf_info(input2.to_string_lossy().to_string()).unwrap();
        let expected_pages = info1.pages + info2.pages;

        let result = combine_pdfs(
            vec![
                input1.to_string_lossy().to_string(),
                input2.to_string_lossy().to_string(),
            ],
            output.to_string_lossy().to_string(),
        );

        assert!(result.is_ok(), "Failed to combine PDFs: {:?}", result.err());
        assert!(output.exists(), "Output file should exist");

        // Verify output is valid PDF with correct page count
        let doc = Document::load(&output).expect("Output should be valid PDF");
        let actual_pages = doc.get_pages().len() as u32;
        assert_eq!(actual_pages, expected_pages,
            "Combined PDF should have {} pages, got {}", expected_pages, actual_pages);

        // Cleanup
        let _ = fs::remove_file(&output);
    }

    #[test]
    fn test_combine_pdfs_reverse_order() {
        let examples = get_examples_dir();
        let input1 = examples.join("shakespeare_sonnets.pdf");
        let input2 = examples.join("jabberwocky.pdf");
        let output = std::env::temp_dir().join("test_reversed.pdf");

        let result = combine_pdfs(
            vec![
                input1.to_string_lossy().to_string(),
                input2.to_string_lossy().to_string(),
            ],
            output.to_string_lossy().to_string(),
        );

        assert!(result.is_ok(), "Failed to combine PDFs in reverse: {:?}", result.err());

        // Verify output is valid PDF
        let doc = Document::load(&output);
        assert!(doc.is_ok(), "Output should be valid PDF");

        // Cleanup
        let _ = fs::remove_file(&output);
    }

    #[test]
    fn test_combine_pdfs_nonexistent_input() {
        let output = std::env::temp_dir().join("test_fail.pdf");

        let result = combine_pdfs(
            vec!["/nonexistent/file.pdf".to_string()],
            output.to_string_lossy().to_string(),
        );

        assert!(result.is_err(), "Should fail for nonexistent input");
    }
}
