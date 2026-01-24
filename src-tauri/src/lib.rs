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

/// Parse a page range string like "1-3,5,7-10" into a sorted, deduplicated Vec of page numbers.
/// Pages are 1-indexed. Returns an error if any page number is invalid or out of range.
fn parse_page_range(input: &str, max_page: u32) -> Result<Vec<u32>, String> {
    if input.trim().is_empty() {
        return Err("Page range cannot be empty".to_string());
    }

    let mut pages = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if part.contains('-') {
            let bounds: Vec<&str> = part.split('-').collect();
            if bounds.len() != 2 {
                return Err(format!("Invalid range format: {}", part));
            }

            let start: u32 = bounds[0]
                .trim()
                .parse()
                .map_err(|_| format!("Invalid page number: {}", bounds[0].trim()))?;
            let end: u32 = bounds[1]
                .trim()
                .parse()
                .map_err(|_| format!("Invalid page number: {}", bounds[1].trim()))?;

            if start == 0 || end == 0 {
                return Err("Page numbers must be greater than 0".to_string());
            }
            if start > max_page || end > max_page {
                return Err(format!(
                    "Page number out of range (max: {})",
                    max_page
                ));
            }
            if start > end {
                return Err(format!("Invalid range: {} > {}", start, end));
            }

            for page in start..=end {
                pages.push(page);
            }
        } else {
            let page: u32 = part
                .parse()
                .map_err(|_| format!("Invalid page number: {}", part))?;

            if page == 0 {
                return Err("Page numbers must be greater than 0".to_string());
            }
            if page > max_page {
                return Err(format!(
                    "Page number {} out of range (max: {})",
                    page, max_page
                ));
            }

            pages.push(page);
        }
    }

    if pages.is_empty() {
        return Err("No valid pages specified".to_string());
    }

    // Sort and deduplicate
    pages.sort();
    pages.dedup();

    Ok(pages)
}

/// Create a new PDF document containing only the specified pages from the source document.
/// page_indices should be 1-indexed page numbers.
fn create_pdf_from_pages(source_path: &str, page_indices: &[u32]) -> Result<Document, String> {
    let mut source = Document::load(source_path).map_err(|e| format!("Failed to load PDF: {}", e))?;

    // Get all pages from source
    let source_pages = source.get_pages();
    let total_pages = source_pages.len() as u32;

    // Validate page indices
    for &page_num in page_indices {
        if page_num == 0 || page_num > total_pages {
            return Err(format!(
                "Page {} out of range (document has {} pages)",
                page_num, total_pages
            ));
        }
    }

    // Renumber objects to avoid conflicts
    source.renumber_objects_with(1);

    // Collect the page object IDs we want to keep (sorted by page number in source)
    let mut page_ids_to_keep: Vec<ObjectId> = Vec::new();
    for &page_num in page_indices {
        if let Some(&object_id) = source_pages.get(&page_num) {
            page_ids_to_keep.push(object_id);
        }
    }

    // Create output document
    let mut output = Document::with_version("1.5");

    // Collect page objects and their dependencies
    let mut pages_dict: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut all_objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    // First, collect the page objects we want
    for &page_id in &page_ids_to_keep {
        if let Ok(object) = source.get_object(page_id) {
            pages_dict.insert(page_id, object.clone());
        }
    }

    // Collect all objects from source, filtering as needed
    for (object_id, object) in source.objects.iter() {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                if catalog_object.is_none() {
                    catalog_object = Some((*object_id, object.clone()));
                }
            }
            b"Pages" => {
                if pages_object.is_none() {
                    pages_object = Some((*object_id, object.clone()));
                }
            }
            b"Page" => {
                // Only include pages we're keeping
                if pages_dict.contains_key(object_id) {
                    // Will be handled separately
                }
            }
            b"Outlines" | b"Outline" => {} // Skip outlines
            _ => {
                all_objects.insert(*object_id, object.clone());
            }
        }
    }

    // Insert all non-page objects
    for (object_id, object) in all_objects {
        output.objects.insert(object_id, object);
    }

    let (pages_id, pages_obj) = pages_object.ok_or("No Pages object found in PDF")?;
    let (catalog_id, catalog_obj) = catalog_object.ok_or("No Catalog object found in PDF")?;

    // Insert page objects with updated parent reference
    for (object_id, object) in pages_dict.iter() {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_id);
            output.objects.insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    // Build new Pages object with only our selected pages
    if let Ok(dictionary) = pages_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", page_ids_to_keep.len() as u32);
        dictionary.set(
            "Kids",
            page_ids_to_keep
                .iter()
                .map(|&id| Object::Reference(id))
                .collect::<Vec<_>>(),
        );
        output.objects.insert(pages_id, Object::Dictionary(dictionary));
    }

    // Build new Catalog object
    if let Ok(dictionary) = catalog_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_id);
        dictionary.remove(b"Outlines");
        output.objects.insert(catalog_id, Object::Dictionary(dictionary));
    }

    // Set trailer
    output.trailer.set("Root", catalog_id);
    output.max_id = output.objects.len() as u32;
    output.renumber_objects();

    Ok(output)
}

#[tauri::command]
fn extract_pages(path: String, pages: String, output_path: String) -> Result<String, String> {
    let doc = Document::load(&path).map_err(|e| format!("Failed to load PDF: {}", e))?;
    let total_pages = doc.get_pages().len() as u32;
    drop(doc); // Close the document before reopening in create_pdf_from_pages

    let page_indices = parse_page_range(&pages, total_pages)?;

    let mut output_doc = create_pdf_from_pages(&path, &page_indices)?;
    output_doc
        .save(&output_path)
        .map_err(|e| format!("Failed to save PDF: {}", e))?;

    Ok(output_path)
}

#[tauri::command]
fn split_to_individual(path: String, output_dir: String) -> Result<Vec<String>, String> {
    let doc = Document::load(&path).map_err(|e| format!("Failed to load PDF: {}", e))?;
    let total_pages = doc.get_pages().len() as u32;
    drop(doc);

    if total_pages == 0 {
        return Err("PDF has no pages".to_string());
    }

    // Get base filename without extension
    let base_name = Path::new(&path)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("page");

    let mut output_paths = Vec::new();

    for page_num in 1..=total_pages {
        let output_path = Path::new(&output_dir)
            .join(format!("{}_page_{}.pdf", base_name, page_num))
            .to_string_lossy()
            .to_string();

        let mut output_doc = create_pdf_from_pages(&path, &[page_num])?;
        output_doc
            .save(&output_path)
            .map_err(|e| format!("Failed to save page {}: {}", page_num, e))?;

        output_paths.push(output_path);
    }

    Ok(output_paths)
}

#[tauri::command]
fn split_at_page(path: String, page: u32, output_dir: String) -> Result<Vec<String>, String> {
    let doc = Document::load(&path).map_err(|e| format!("Failed to load PDF: {}", e))?;
    let total_pages = doc.get_pages().len() as u32;
    drop(doc);

    if page == 0 {
        return Err("Page number must be greater than 0".to_string());
    }
    if page >= total_pages {
        return Err(format!(
            "Cannot split after page {} (document has {} pages)",
            page, total_pages
        ));
    }

    // Get base filename without extension
    let base_name = Path::new(&path)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("split");

    let part1_path = Path::new(&output_dir)
        .join(format!("{}_part1.pdf", base_name))
        .to_string_lossy()
        .to_string();
    let part2_path = Path::new(&output_dir)
        .join(format!("{}_part2.pdf", base_name))
        .to_string_lossy()
        .to_string();

    // Create part 1: pages 1 to page
    let part1_pages: Vec<u32> = (1..=page).collect();
    let mut part1_doc = create_pdf_from_pages(&path, &part1_pages)?;
    part1_doc
        .save(&part1_path)
        .map_err(|e| format!("Failed to save part 1: {}", e))?;

    // Create part 2: pages page+1 to total
    let part2_pages: Vec<u32> = ((page + 1)..=total_pages).collect();
    let mut part2_doc = create_pdf_from_pages(&path, &part2_pages)?;
    part2_doc
        .save(&part2_path)
        .map_err(|e| format!("Failed to save part 2: {}", e))?;

    Ok(vec![part1_path, part2_path])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_pdf_info,
            combine_pdfs,
            extract_pages,
            split_to_individual,
            split_at_page
        ])
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

    // Tests for parse_page_range
    #[test]
    fn test_parse_page_range_single_page() {
        let result = parse_page_range("1", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1]);
    }

    #[test]
    fn test_parse_page_range_multiple_pages() {
        let result = parse_page_range("1,3,5", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 3, 5]);
    }

    #[test]
    fn test_parse_page_range_range() {
        let result = parse_page_range("1-3", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_parse_page_range_mixed() {
        let result = parse_page_range("1-3, 5, 7-9", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3, 5, 7, 8, 9]);
    }

    #[test]
    fn test_parse_page_range_with_spaces() {
        let result = parse_page_range("  1 - 3 , 5 ", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3, 5]);
    }

    #[test]
    fn test_parse_page_range_deduplicates() {
        let result = parse_page_range("1,2,1,2,3", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_parse_page_range_sorts() {
        let result = parse_page_range("5,1,3", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 3, 5]);
    }

    #[test]
    fn test_parse_page_range_empty() {
        let result = parse_page_range("", 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_parse_page_range_zero() {
        let result = parse_page_range("0", 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("greater than 0"));
    }

    #[test]
    fn test_parse_page_range_out_of_bounds() {
        let result = parse_page_range("999", 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of range"));
    }

    #[test]
    fn test_parse_page_range_invalid_format() {
        let result = parse_page_range("abc", 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid page number"));
    }

    #[test]
    fn test_parse_page_range_invalid_range() {
        let result = parse_page_range("5-3", 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("5 > 3"));
    }

    // Tests for extract_pages
    #[test]
    fn test_extract_pages_single() {
        let examples = get_examples_dir();
        let input = examples.join("shakespeare_sonnets.pdf");
        let output = std::env::temp_dir().join("test_extract_single.pdf");

        let info = get_pdf_info(input.to_string_lossy().to_string()).unwrap();
        assert!(info.pages >= 1, "Test PDF should have at least 1 page");

        let result = extract_pages(
            input.to_string_lossy().to_string(),
            "1".to_string(),
            output.to_string_lossy().to_string(),
        );

        assert!(result.is_ok(), "Failed to extract page: {:?}", result.err());
        assert!(output.exists(), "Output file should exist");

        let doc = Document::load(&output).expect("Output should be valid PDF");
        assert_eq!(doc.get_pages().len(), 1, "Should have exactly 1 page");

        let _ = fs::remove_file(&output);
    }

    #[test]
    fn test_extract_pages_range() {
        let examples = get_examples_dir();
        let input1 = examples.join("jabberwocky.pdf");
        let input2 = examples.join("shakespeare_sonnets.pdf");
        let combined = std::env::temp_dir().join("test_extract_range_combined.pdf");
        let output = std::env::temp_dir().join("test_extract_range.pdf");

        // First create a combined PDF with multiple pages
        let info1 = get_pdf_info(input1.to_string_lossy().to_string()).unwrap();
        let info2 = get_pdf_info(input2.to_string_lossy().to_string()).unwrap();
        let total_pages = info1.pages + info2.pages;

        combine_pdfs(
            vec![
                input1.to_string_lossy().to_string(),
                input2.to_string_lossy().to_string(),
            ],
            combined.to_string_lossy().to_string(),
        )
        .unwrap();

        assert!(total_pages >= 2, "Combined PDF should have at least 2 pages");

        // Extract pages 1 to total
        let pages_to_extract = format!("1-{}", total_pages);
        let result = extract_pages(
            combined.to_string_lossy().to_string(),
            pages_to_extract,
            output.to_string_lossy().to_string(),
        );

        assert!(result.is_ok(), "Failed to extract pages: {:?}", result.err());
        assert!(output.exists(), "Output file should exist");

        let doc = Document::load(&output).expect("Output should be valid PDF");
        assert_eq!(
            doc.get_pages().len() as u32,
            total_pages,
            "Should have {} pages",
            total_pages
        );

        let _ = fs::remove_file(&output);
        let _ = fs::remove_file(&combined);
    }

    #[test]
    fn test_extract_pages_invalid_range() {
        let examples = get_examples_dir();
        let input = examples.join("jabberwocky.pdf");
        let output = std::env::temp_dir().join("test_extract_invalid.pdf");

        let result = extract_pages(
            input.to_string_lossy().to_string(),
            "999".to_string(),
            output.to_string_lossy().to_string(),
        );

        assert!(result.is_err(), "Should fail for invalid page range");
    }

    // Tests for split_to_individual
    #[test]
    fn test_split_to_individual() {
        let examples = get_examples_dir();
        let input = examples.join("shakespeare_sonnets.pdf");
        let output_dir = std::env::temp_dir().join("test_split_individual");

        // Create output directory
        let _ = fs::create_dir_all(&output_dir);

        let info = get_pdf_info(input.to_string_lossy().to_string()).unwrap();
        let expected_files = info.pages as usize;

        let result = split_to_individual(
            input.to_string_lossy().to_string(),
            output_dir.to_string_lossy().to_string(),
        );

        assert!(result.is_ok(), "Failed to split PDF: {:?}", result.err());
        let paths = result.unwrap();
        assert_eq!(paths.len(), expected_files, "Should create {} files", expected_files);

        // Verify each file exists and has 1 page
        for path in &paths {
            let path_buf = std::path::PathBuf::from(path);
            assert!(path_buf.exists(), "File should exist: {}", path);

            let doc = Document::load(&path_buf).expect("Should be valid PDF");
            assert_eq!(doc.get_pages().len(), 1, "Each split file should have 1 page");
        }

        // Cleanup
        let _ = fs::remove_dir_all(&output_dir);
    }

    // Tests for split_at_page
    #[test]
    fn test_split_at_page() {
        let examples = get_examples_dir();
        let input1 = examples.join("jabberwocky.pdf");
        let input2 = examples.join("shakespeare_sonnets.pdf");
        let combined = std::env::temp_dir().join("test_split_at_combined.pdf");
        let output_dir = std::env::temp_dir().join("test_split_at_page");

        let _ = fs::create_dir_all(&output_dir);

        // First create a combined PDF with multiple pages
        let info1 = get_pdf_info(input1.to_string_lossy().to_string()).unwrap();
        let info2 = get_pdf_info(input2.to_string_lossy().to_string()).unwrap();
        let total_pages = info1.pages + info2.pages;

        combine_pdfs(
            vec![
                input1.to_string_lossy().to_string(),
                input2.to_string_lossy().to_string(),
            ],
            combined.to_string_lossy().to_string(),
        )
        .unwrap();

        assert!(total_pages >= 2, "Combined PDF should have at least 2 pages");

        let split_point = 1u32;

        let result = split_at_page(
            combined.to_string_lossy().to_string(),
            split_point,
            output_dir.to_string_lossy().to_string(),
        );

        assert!(result.is_ok(), "Failed to split PDF: {:?}", result.err());
        let paths = result.unwrap();
        assert_eq!(paths.len(), 2, "Should create 2 files");

        // Verify part 1 has correct page count
        let part1 = Document::load(&paths[0]).expect("Part 1 should be valid PDF");
        assert_eq!(
            part1.get_pages().len() as u32,
            split_point,
            "Part 1 should have {} pages",
            split_point
        );

        // Verify part 2 has correct page count
        let part2 = Document::load(&paths[1]).expect("Part 2 should be valid PDF");
        assert_eq!(
            part2.get_pages().len() as u32,
            total_pages - split_point,
            "Part 2 should have {} pages",
            total_pages - split_point
        );

        // Cleanup
        let _ = fs::remove_dir_all(&output_dir);
        let _ = fs::remove_file(&combined);
    }

    #[test]
    fn test_split_at_page_zero() {
        let examples = get_examples_dir();
        let input = examples.join("shakespeare_sonnets.pdf");
        let output_dir = std::env::temp_dir();

        let result = split_at_page(
            input.to_string_lossy().to_string(),
            0,
            output_dir.to_string_lossy().to_string(),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("greater than 0"));
    }

    #[test]
    fn test_split_at_page_last_page() {
        let examples = get_examples_dir();
        let input1 = examples.join("jabberwocky.pdf");
        let input2 = examples.join("shakespeare_sonnets.pdf");
        let combined = std::env::temp_dir().join("test_split_last_combined.pdf");
        let output_dir = std::env::temp_dir();

        // Create a combined PDF
        let info1 = get_pdf_info(input1.to_string_lossy().to_string()).unwrap();
        let info2 = get_pdf_info(input2.to_string_lossy().to_string()).unwrap();
        let total_pages = info1.pages + info2.pages;

        combine_pdfs(
            vec![
                input1.to_string_lossy().to_string(),
                input2.to_string_lossy().to_string(),
            ],
            combined.to_string_lossy().to_string(),
        )
        .unwrap();

        // Try to split after the last page (invalid - nothing for part 2)
        let result = split_at_page(
            combined.to_string_lossy().to_string(),
            total_pages,
            output_dir.to_string_lossy().to_string(),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot split"));

        let _ = fs::remove_file(&combined);
    }
}
