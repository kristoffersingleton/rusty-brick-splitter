<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open, save } from "@tauri-apps/plugin-dialog";

  interface PdfFile {
    path: string;
    name: string;
    pages: number;
  }

  // Main mode: combine or split
  let mode = $state<"combine" | "split">("combine");

  // Split sub-mode
  let splitMode = $state<"extract" | "individual" | "at-page">("extract");

  // Combine mode state
  let files = $state<PdfFile[]>([]);

  // Split mode state
  let selectedFile = $state<PdfFile | null>(null);
  let pageRangeInput = $state("");
  let splitAfterPage = $state(1);

  // Shared state
  let status = $state("");
  let isProcessing = $state(false);
  let draggedIndex = $state<number | null>(null);
  let dragOverIndex = $state<number | null>(null);

  async function addFiles() {
    const selected = await open({
      multiple: mode === "combine",
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });

    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      for (const path of paths) {
        try {
          const info = await invoke<PdfFile>("get_pdf_info", { path });
          if (mode === "combine") {
            files = [...files, info];
          } else {
            selectedFile = info;
            // Reset split-specific state when new file is selected
            pageRangeInput = "";
            splitAfterPage = 1;
          }
        } catch (e) {
          status = `Error loading ${path}: ${e}`;
        }
      }
    }
  }

  function removeFile(index: number) {
    files = files.filter((_, i) => i !== index);
  }

  function clearSelectedFile() {
    selectedFile = null;
    pageRangeInput = "";
    splitAfterPage = 1;
  }

  function moveUp(index: number) {
    if (index > 0) {
      const newFiles = [...files];
      [newFiles[index - 1], newFiles[index]] = [newFiles[index], newFiles[index - 1]];
      files = newFiles;
    }
  }

  function moveDown(index: number) {
    if (index < files.length - 1) {
      const newFiles = [...files];
      [newFiles[index], newFiles[index + 1]] = [newFiles[index + 1], newFiles[index]];
      files = newFiles;
    }
  }

  function handleDragStart(index: number) {
    draggedIndex = index;
  }

  function handleDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    dragOverIndex = index;
  }

  function handleDrop(index: number) {
    if (draggedIndex !== null && draggedIndex !== index) {
      const newFiles = [...files];
      const [removed] = newFiles.splice(draggedIndex, 1);
      newFiles.splice(index, 0, removed);
      files = newFiles;
    }
    draggedIndex = null;
    dragOverIndex = null;
  }

  function handleDragEnd() {
    draggedIndex = null;
    dragOverIndex = null;
  }

  async function combinePdfs() {
    if (files.length < 2) {
      status = "Add at least 2 PDF files to combine";
      return;
    }

    const outputPath = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: "combined.pdf",
    });

    if (!outputPath) return;

    isProcessing = true;
    status = "Combining PDFs...";

    try {
      const paths = files.map((f) => f.path);
      await invoke("combine_pdfs", { paths, outputPath });
      status = `Successfully saved to ${outputPath}`;
    } catch (e) {
      status = `Error: ${e}`;
    } finally {
      isProcessing = false;
    }
  }

  async function extractPages() {
    if (!selectedFile) {
      status = "Please select a PDF file first";
      return;
    }

    if (!pageRangeInput.trim()) {
      status = "Please enter page numbers to extract";
      return;
    }

    const outputPath = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: `${selectedFile.name.replace(".pdf", "")}_extracted.pdf`,
    });

    if (!outputPath) return;

    isProcessing = true;
    status = "Extracting pages...";

    try {
      await invoke("extract_pages", {
        path: selectedFile.path,
        pages: pageRangeInput,
        outputPath,
      });
      status = `Successfully saved to ${outputPath}`;
    } catch (e) {
      status = `Error: ${e}`;
    } finally {
      isProcessing = false;
    }
  }

  async function splitToIndividual() {
    if (!selectedFile) {
      status = "Please select a PDF file first";
      return;
    }

    const outputDir = await open({
      directory: true,
      title: "Select output folder",
    });

    if (!outputDir) return;

    isProcessing = true;
    status = "Splitting PDF into individual pages...";

    try {
      const paths = await invoke<string[]>("split_to_individual", {
        path: selectedFile.path,
        outputDir,
      });
      status = `Successfully created ${paths.length} files in ${outputDir}`;
    } catch (e) {
      status = `Error: ${e}`;
    } finally {
      isProcessing = false;
    }
  }

  async function splitAtPage() {
    if (!selectedFile) {
      status = "Please select a PDF file first";
      return;
    }

    if (splitAfterPage < 1 || splitAfterPage >= selectedFile.pages) {
      status = `Please enter a valid page number (1-${selectedFile.pages - 1})`;
      return;
    }

    const outputDir = await open({
      directory: true,
      title: "Select output folder",
    });

    if (!outputDir) return;

    isProcessing = true;
    status = "Splitting PDF...";

    try {
      const paths = await invoke<string[]>("split_at_page", {
        path: selectedFile.path,
        page: splitAfterPage,
        outputDir,
      });
      status = `Successfully created ${paths.length} files in ${outputDir}`;
    } catch (e) {
      status = `Error: ${e}`;
    } finally {
      isProcessing = false;
    }
  }

  function clearAll() {
    files = [];
    selectedFile = null;
    pageRangeInput = "";
    splitAfterPage = 1;
    status = "";
  }

  function switchMode(newMode: "combine" | "split") {
    mode = newMode;
    status = "";
  }

  const totalPages = $derived(files.reduce((sum, f) => sum + f.pages, 0));
</script>

<main class="min-h-screen bg-zinc-900 text-zinc-100 p-8">
  <div class="max-w-2xl mx-auto">
    <header class="mb-8">
      <h1 class="text-3xl font-bold text-white mb-2">PDF Tools</h1>
      <p class="text-zinc-400">Combine or split your PDF files.</p>
    </header>

    <!-- Mode Toggle -->
    <div class="flex mb-6 bg-zinc-800 rounded-lg p-1">
      <button
        onclick={() => switchMode("combine")}
        class="flex-1 px-4 py-2 rounded-md font-medium transition-colors
          {mode === 'combine' ? 'bg-blue-600 text-white' : 'text-zinc-400 hover:text-white'}"
      >
        Combine
      </button>
      <button
        onclick={() => switchMode("split")}
        class="flex-1 px-4 py-2 rounded-md font-medium transition-colors
          {mode === 'split' ? 'bg-blue-600 text-white' : 'text-zinc-400 hover:text-white'}"
      >
        Split
      </button>
    </div>

    {#if mode === "combine"}
      <!-- Combine Mode -->
      <div class="flex gap-3 mb-6">
        <button
          onclick={addFiles}
          class="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg font-medium transition-colors"
        >
          + Add PDFs
        </button>
        {#if files.length > 0}
          <button
            onclick={clearAll}
            class="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 rounded-lg font-medium transition-colors"
          >
            Clear All
          </button>
        {/if}
      </div>

      {#if files.length === 0}
        <div
          class="border-2 border-dashed border-zinc-700 rounded-xl p-12 text-center text-zinc-500"
        >
          <p class="text-lg mb-2">No PDFs added yet</p>
          <p class="text-sm">Click "Add PDFs" to get started</p>
        </div>
      {:else}
        <div class="space-y-2 mb-6" role="list">
          {#each files as file, index}
            <div
              role="listitem"
              draggable="true"
              ondragstart={() => handleDragStart(index)}
              ondragover={(e) => handleDragOver(e, index)}
              ondrop={() => handleDrop(index)}
              ondragend={handleDragEnd}
              class="flex items-center gap-3 p-3 bg-zinc-800 rounded-lg border transition-all cursor-move
                {draggedIndex === index ? 'opacity-50 border-zinc-600' : 'border-zinc-700'}
                {dragOverIndex === index && draggedIndex !== index ? 'border-blue-500 bg-zinc-750' : ''}"
            >
              <span class="text-zinc-500 font-mono text-sm w-6">{index + 1}</span>

              <div class="flex-1 min-w-0">
                <p class="font-medium truncate">{file.name}</p>
                <p class="text-sm text-zinc-500">{file.pages} page{file.pages !== 1 ? 's' : ''}</p>
              </div>

              <div class="flex items-center gap-1">
                <button
                  onclick={() => moveUp(index)}
                  disabled={index === 0}
                  class="p-1.5 rounded hover:bg-zinc-700 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                  title="Move up"
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7" />
                  </svg>
                </button>
                <button
                  onclick={() => moveDown(index)}
                  disabled={index === files.length - 1}
                  class="p-1.5 rounded hover:bg-zinc-700 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                  title="Move down"
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                  </svg>
                </button>
                <button
                  onclick={() => removeFile(index)}
                  class="p-1.5 rounded hover:bg-red-900/50 text-zinc-400 hover:text-red-400 transition-colors ml-2"
                  title="Remove"
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            </div>
          {/each}
        </div>

        <div class="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg mb-6">
          <span class="text-zinc-400">
            {files.length} file{files.length !== 1 ? 's' : ''} &middot; {totalPages} total pages
          </span>
          <button
            onclick={combinePdfs}
            disabled={isProcessing || files.length < 2}
            class="px-6 py-2 bg-green-600 hover:bg-green-500 disabled:bg-zinc-700 disabled:text-zinc-500
              rounded-lg font-medium transition-colors disabled:cursor-not-allowed"
          >
            {isProcessing ? 'Combining...' : 'Combine PDFs'}
          </button>
        </div>
      {/if}
    {:else}
      <!-- Split Mode -->
      <!-- Split Sub-mode Tabs -->
      <div class="flex gap-2 mb-6">
        <button
          onclick={() => (splitMode = "extract")}
          class="px-3 py-1.5 rounded-md text-sm font-medium transition-colors
            {splitMode === 'extract' ? 'bg-zinc-700 text-white' : 'text-zinc-400 hover:text-white'}"
        >
          Extract Pages
        </button>
        <button
          onclick={() => (splitMode = "individual")}
          class="px-3 py-1.5 rounded-md text-sm font-medium transition-colors
            {splitMode === 'individual' ? 'bg-zinc-700 text-white' : 'text-zinc-400 hover:text-white'}"
        >
          Split to Individual
        </button>
        <button
          onclick={() => (splitMode = "at-page")}
          class="px-3 py-1.5 rounded-md text-sm font-medium transition-colors
            {splitMode === 'at-page' ? 'bg-zinc-700 text-white' : 'text-zinc-400 hover:text-white'}"
        >
          Split at Page
        </button>
      </div>

      <div class="flex gap-3 mb-6">
        <button
          onclick={addFiles}
          class="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg font-medium transition-colors"
        >
          {selectedFile ? "Change PDF" : "+ Select PDF"}
        </button>
        {#if selectedFile}
          <button
            onclick={clearSelectedFile}
            class="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 rounded-lg font-medium transition-colors"
          >
            Clear
          </button>
        {/if}
      </div>

      {#if !selectedFile}
        <div
          class="border-2 border-dashed border-zinc-700 rounded-xl p-12 text-center text-zinc-500"
        >
          <p class="text-lg mb-2">No PDF selected</p>
          <p class="text-sm">Click "Select PDF" to choose a file to split</p>
        </div>
      {:else}
        <!-- Selected file card -->
        <div class="p-4 bg-zinc-800 rounded-lg border border-zinc-700 mb-6">
          <div class="flex items-center gap-3">
            <svg class="w-8 h-8 text-red-400" fill="currentColor" viewBox="0 0 24 24">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6zm-1 2l5 5h-5V4zM8.5 13h2v5h-2v-5zm3 0h2v5h-2v-5z"/>
            </svg>
            <div class="flex-1 min-w-0">
              <p class="font-medium truncate">{selectedFile.name}</p>
              <p class="text-sm text-zinc-500">{selectedFile.pages} page{selectedFile.pages !== 1 ? 's' : ''}</p>
            </div>
          </div>
        </div>

        {#if splitMode === "extract"}
          <!-- Extract Pages UI -->
          <div class="space-y-4 mb-6">
            <div>
              <label for="pageRange" class="block text-sm font-medium text-zinc-300 mb-2">
                Page numbers to extract
              </label>
              <input
                id="pageRange"
                type="text"
                bind:value={pageRangeInput}
                placeholder="e.g., 1-3, 5, 7-10"
                class="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white
                  placeholder-zinc-500 focus:outline-none focus:border-blue-500"
              />
              <p class="mt-1 text-xs text-zinc-500">
                Enter page numbers or ranges separated by commas (e.g., 1-3, 5, 7-10)
              </p>
            </div>

            <button
              onclick={extractPages}
              disabled={isProcessing || !pageRangeInput.trim()}
              class="w-full px-6 py-2 bg-green-600 hover:bg-green-500 disabled:bg-zinc-700 disabled:text-zinc-500
                rounded-lg font-medium transition-colors disabled:cursor-not-allowed"
            >
              {isProcessing ? 'Extracting...' : 'Extract Pages'}
            </button>
          </div>
        {:else if splitMode === "individual"}
          <!-- Split to Individual UI -->
          <div class="space-y-4 mb-6">
            <div class="p-4 bg-zinc-800/50 rounded-lg">
              <p class="text-zinc-300">
                This will create <span class="font-bold text-white">{selectedFile.pages}</span> separate PDF files,
                one for each page.
              </p>
            </div>

            <button
              onclick={splitToIndividual}
              disabled={isProcessing}
              class="w-full px-6 py-2 bg-green-600 hover:bg-green-500 disabled:bg-zinc-700 disabled:text-zinc-500
                rounded-lg font-medium transition-colors disabled:cursor-not-allowed"
            >
              {isProcessing ? 'Splitting...' : 'Split All Pages'}
            </button>
          </div>
        {:else if splitMode === "at-page"}
          <!-- Split at Page UI -->
          <div class="space-y-4 mb-6">
            <div>
              <label for="splitPage" class="block text-sm font-medium text-zinc-300 mb-2">
                Split after page
              </label>
              <input
                id="splitPage"
                type="number"
                bind:value={splitAfterPage}
                min="1"
                max={selectedFile.pages - 1}
                class="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white
                  placeholder-zinc-500 focus:outline-none focus:border-blue-500"
              />
            </div>

            {#if splitAfterPage >= 1 && splitAfterPage < selectedFile.pages}
              <div class="p-4 bg-zinc-800/50 rounded-lg space-y-2">
                <p class="text-zinc-300">
                  <span class="font-medium text-white">Part 1:</span> Pages 1-{splitAfterPage}
                  ({splitAfterPage} page{splitAfterPage !== 1 ? 's' : ''})
                </p>
                <p class="text-zinc-300">
                  <span class="font-medium text-white">Part 2:</span> Pages {splitAfterPage + 1}-{selectedFile.pages}
                  ({selectedFile.pages - splitAfterPage} page{selectedFile.pages - splitAfterPage !== 1 ? 's' : ''})
                </p>
              </div>
            {/if}

            <button
              onclick={splitAtPage}
              disabled={isProcessing || splitAfterPage < 1 || splitAfterPage >= selectedFile.pages}
              class="w-full px-6 py-2 bg-green-600 hover:bg-green-500 disabled:bg-zinc-700 disabled:text-zinc-500
                rounded-lg font-medium transition-colors disabled:cursor-not-allowed"
            >
              {isProcessing ? 'Splitting...' : 'Split PDF'}
            </button>
          </div>
        {/if}
      {/if}
    {/if}

    {#if status}
      <div
        class="p-3 rounded-lg text-sm {status.startsWith('Error') || status.startsWith('Please')
          ? 'bg-red-900/30 text-red-300'
          : 'bg-green-900/30 text-green-300'}"
      >
        {status}
      </div>
    {/if}
  </div>
</main>
