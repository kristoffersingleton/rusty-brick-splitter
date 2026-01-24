<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open, save } from "@tauri-apps/plugin-dialog";

  interface PdfFile {
    path: string;
    name: string;
    pages: number;
  }

  let files = $state<PdfFile[]>([]);
  let status = $state("");
  let isProcessing = $state(false);
  let draggedIndex = $state<number | null>(null);
  let dragOverIndex = $state<number | null>(null);

  async function addFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });

    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      for (const path of paths) {
        try {
          const info = await invoke<PdfFile>("get_pdf_info", { path });
          files = [...files, info];
        } catch (e) {
          status = `Error loading ${path}: ${e}`;
        }
      }
    }
  }

  function removeFile(index: number) {
    files = files.filter((_, i) => i !== index);
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

  function clearAll() {
    files = [];
    status = "";
  }

  const totalPages = $derived(files.reduce((sum, f) => sum + f.pages, 0));
</script>

<main class="min-h-screen bg-zinc-900 text-zinc-100 p-8">
  <div class="max-w-2xl mx-auto">
    <header class="mb-8">
      <h1 class="text-3xl font-bold text-white mb-2">PDF Combiner</h1>
      <p class="text-zinc-400">Select PDFs, reorder them, combine into one.</p>
    </header>

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

    {#if status}
      <div
        class="p-3 rounded-lg text-sm {status.startsWith('Error') || status.startsWith('Add at least')
          ? 'bg-red-900/30 text-red-300'
          : 'bg-green-900/30 text-green-300'}"
      >
        {status}
      </div>
    {/if}
  </div>
</main>
