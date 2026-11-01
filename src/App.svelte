<script lang="ts">
  import { onMount } from 'svelte';

  interface ProcessBrief {
    pid: number;
    name: string;
    memory_mb: number;
    exe_path: string | null;
  }

  interface SystemStats {
    cpu_usage_percent: number;
    memory_used_mb: number;
    memory_total_mb: number;
    top_memory_processes: ProcessBrief[];
  }

  interface ListeningPort {
    port: number;
    protocol: string;
    pid: number;
    process_name: string;
    exe_path: string | null;
    is_dev_port: boolean;
  }

  let activeTab = $state<'ports' | 'processes'>('ports');
  let searchQuery = $state('');
  let refreshInterval = $state<'1s' | '3s' | 'pause'>('1s');
  let isLoading = $state(false);
  let errorMsg = $state<string | null>(null);
  let isBrowserMode = $state(false);

  let stats = $state<SystemStats | null>(null);
  let ports = $state<ListeningPort[]>([]);

  let killConfirmPid = $state<number | null>(null);
  let confirmTimeout = $state<any>(null);
  let copiedPort = $state<number | null>(null);
  let timerId: any = null;

  function checkIsTauri(): boolean {
    return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
  }

  const MOCK_STATS: SystemStats = {
    cpu_usage_percent: 14.2,
    memory_used_mb: 6340,
    memory_total_mb: 16384,
    top_memory_processes: [
      { pid: 14208, name: 'node.exe', memory_mb: 482, exe_path: 'C:\\Program Files\\nodejs\\node.exe' },
      { pid: 9812, name: 'chrome.exe', memory_mb: 395, exe_path: 'C:\\Program Files\\Google\\Chrome\\chrome.exe' },
      { pid: 5410, name: 'python.exe', memory_mb: 284, exe_path: 'C:\\Python311\\python.exe' },
      { pid: 1204, name: 'mysqld.exe', memory_mb: 210, exe_path: 'C:\\Program Files\\MySQL\\mysqld.exe' },
      { pid: 8490, name: 'docker.exe', memory_mb: 175, exe_path: 'C:\\Program Files\\Docker\\docker.exe' },
    ],
  };

  const MOCK_PORTS: ListeningPort[] = [
    { port: 5173, protocol: 'TCP', pid: 14208, process_name: 'node.exe (Vite Dev)', exe_path: 'd:\\Project\\Dev Monitor', is_dev_port: true },
    { port: 3000, protocol: 'TCP', pid: 14208, process_name: 'node.exe (Next.js)', exe_path: 'C:\\Projects\\app', is_dev_port: true },
    { port: 8000, protocol: 'TCP', pid: 5410, process_name: 'python.exe (FastAPI)', exe_path: 'C:\\Python311\\python.exe', is_dev_port: true },
    { port: 3306, protocol: 'TCP', pid: 1204, process_name: 'mysqld.exe', exe_path: 'C:\\Program Files\\MySQL\\mysqld.exe', is_dev_port: true },
    { port: 135, protocol: 'TCP', pid: 988, process_name: 'svchost.exe', exe_path: 'C:\\Windows\\System32\\svchost.exe', is_dev_port: false },
  ];

  async function fetchStats() {
    if (!checkIsTauri()) {
      isBrowserMode = true;
      stats = MOCK_STATS;
      return;
    }
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const data = await invoke<SystemStats>('get_system_stats');
      stats = data;
    } catch (e: any) {
      console.error('Error fetching system stats:', e);
      if (checkIsTauri()) errorMsg = String(e);
    }
  }

  async function fetchPorts() {
    if (!checkIsTauri()) {
      isBrowserMode = true;
      ports = MOCK_PORTS;
      return;
    }
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const data = await invoke<ListeningPort[]>('get_active_ports');
      ports = data;
    } catch (e: any) {
      console.error('Error fetching active ports:', e);
      if (checkIsTauri()) errorMsg = String(e);
    }
  }

  async function refreshAll() {
    isLoading = true;
    errorMsg = null;
    await Promise.all([fetchStats(), fetchPorts()]);
    isLoading = false;
  }

  function handleIntervalChange(mode: '1s' | '3s' | 'pause') {
    refreshInterval = mode;
    resetTimer();
  }

  function resetTimer() {
    if (timerId) clearInterval(timerId);
    if (refreshInterval === '1s') {
      timerId = setInterval(refreshAll, 1000);
    } else if (refreshInterval === '3s') {
      timerId = setInterval(refreshAll, 3000);
    }
  }

  async function handleKill(pid: number) {
    if (killConfirmPid !== pid) {
      killConfirmPid = pid;
      if (confirmTimeout) clearTimeout(confirmTimeout);
      confirmTimeout = setTimeout(() => {
        killConfirmPid = null;
      }, 3500);
      return;
    }

    killConfirmPid = null;
    if (!checkIsTauri()) {
      ports = ports.filter(p => p.pid !== pid);
      if (stats) {
        stats.top_memory_processes = stats.top_memory_processes.filter(p => p.pid !== pid);
      }
      return;
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('kill_process', { pid });
      await refreshAll();
    } catch (e: any) {
      errorMsg = `Failed to terminate PID ${pid}: ${e}`;
    }
  }

  async function handleOpenBrowser(port: number) {
    const url = `http://localhost:${port}`;
    if (checkIsTauri()) {
      try {
        const { openUrl } = await import('@tauri-apps/plugin-opener');
        await openUrl(url);
        return;
      } catch (e) {
        console.warn('Fallback to window.open:', e);
      }
    }
    window.open(url, '_blank');
  }

  async function handleCopyUri(port: number) {
    const uri = `localhost:${port}`;
    try {
      await navigator.clipboard.writeText(uri);
      copiedPort = port;
      setTimeout(() => {
        if (copiedPort === port) copiedPort = null;
      }, 2000);
    } catch (e) {
      console.error('Failed to copy to clipboard:', e);
    }
  }

  async function handleHideWindow() {
    if (checkIsTauri()) {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await getCurrentWindow().hide();
      } catch (e) {
        console.error('Error hiding window:', e);
      }
    }
  }

  function formatBytes(mb: number): string {
    if (mb >= 1024) {
      return (mb / 1024).toFixed(1) + ' GB';
    }
    return mb + ' MB';
  }

  let filteredPorts = $derived(
    ports.filter((p) => {
      if (!searchQuery.trim()) return true;
      const q = searchQuery.toLowerCase();
      return (
        p.port.toString().includes(q) ||
        p.process_name.toLowerCase().includes(q) ||
        (p.exe_path && p.exe_path.toLowerCase().includes(q))
      );
    })
  );

  let devPortsCount = $derived(ports.filter((p) => p.is_dev_port).length);

  onMount(() => {
    refreshAll();
    resetTimer();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        handleHideWindow();
      }
    };
    window.addEventListener('keydown', handleKeyDown);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      if (timerId) clearInterval(timerId);
    };
  });
</script>

<main class="flex flex-col h-screen w-screen bg-slate-950 text-slate-100 font-mono text-xs select-none">
  <!-- Top App Header / System Metrics -->
  <header class="p-3 bg-slate-900/90 border-b border-slate-800 flex flex-col gap-2 backdrop-blur-md">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2">
        <span class="relative flex h-2 w-2">
          <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
          <span class="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
        </span>
        <h1 class="font-bold tracking-tight text-sm bg-gradient-to-r from-emerald-400 to-cyan-400 bg-clip-text text-transparent">
          PORT SENTINEL
        </h1>
      </div>

      <!-- Controls: Refresh Interval + Manual Refresh -->
      <div class="flex items-center gap-1.5 bg-slate-950/80 p-1 rounded-lg border border-slate-800">
        <button
          onclick={() => handleIntervalChange('1s')}
          class="px-1.5 py-0.5 rounded text-[10px] font-semibold transition-colors {refreshInterval === '1s'
            ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
            : 'text-slate-400 hover:text-slate-200'}"
        >
          1s
        </button>
        <button
          onclick={() => handleIntervalChange('3s')}
          class="px-1.5 py-0.5 rounded text-[10px] font-semibold transition-colors {refreshInterval === '3s'
            ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
            : 'text-slate-400 hover:text-slate-200'}"
        >
          3s
        </button>
        <button
          onclick={() => handleIntervalChange('pause')}
          class="px-1.5 py-0.5 rounded text-[10px] font-semibold transition-colors {refreshInterval === 'pause'
            ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
            : 'text-slate-400 hover:text-slate-200'}"
        >
          Pause
        </button>
        <div class="w-px h-3 bg-slate-800 mx-0.5"></div>
        <button
          onclick={refreshAll}
          title="Refresh Now"
          class="p-1 rounded text-slate-400 hover:text-slate-100 hover:bg-slate-800 transition-colors"
        >
          <svg
            class="w-3.5 h-3.5 {isLoading ? 'animate-spin text-cyan-400' : ''}"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
            />
          </svg>
        </button>
      </div>
    </div>

    <!-- Telemetry Metrics Bar (CPU & RAM) -->
    <div class="grid grid-cols-2 gap-2 text-[11px]">
      <!-- CPU Usage -->
      <div class="bg-slate-950/70 p-2 rounded-lg border border-slate-800/80 flex flex-col gap-1">
        <div class="flex justify-between items-center text-[10px] text-slate-400">
          <span>CPU LOAD</span>
          <span
            class="font-bold {stats && stats.cpu_usage_percent > 80
              ? 'text-rose-400'
              : stats && stats.cpu_usage_percent > 50
                ? 'text-amber-400'
                : 'text-emerald-400'}"
          >
            {stats ? stats.cpu_usage_percent.toFixed(1) : '0.0'}%
          </span>
        </div>
        <div class="w-full bg-slate-800/80 rounded-full h-1.5 overflow-hidden">
          <div
            class="h-full rounded-full transition-all duration-500 {stats && stats.cpu_usage_percent > 80
              ? 'bg-rose-500'
              : stats && stats.cpu_usage_percent > 50
                ? 'bg-amber-500'
                : 'bg-emerald-500'}"
            style="width: {Math.min(100, Math.max(0, stats ? stats.cpu_usage_percent : 0))}%"
          ></div>
        </div>
      </div>

      <!-- RAM Usage -->
      <div class="bg-slate-950/70 p-2 rounded-lg border border-slate-800/80 flex flex-col gap-1">
        <div class="flex justify-between items-center text-[10px] text-slate-400">
          <span>MEMORY</span>
          <span class="font-bold text-cyan-400">
            {stats ? formatBytes(stats.memory_used_mb) : '0 MB'}
          </span>
        </div>
        <div class="w-full bg-slate-800/80 rounded-full h-1.5 overflow-hidden">
          <div
            class="h-full bg-gradient-to-r from-cyan-500 to-blue-500 rounded-full transition-all duration-500"
            style="width: {stats && stats.memory_total_mb > 0
              ? Math.min(100, (stats.memory_used_mb / stats.memory_total_mb) * 100)
              : 0}%"
          ></div>
        </div>
      </div>
    </div>

    <!-- Navigation Tabs -->
    <div class="flex gap-1 mt-1 border-b border-slate-800/60 pb-0">
      <button
        onclick={() => (activeTab = 'ports')}
        class="px-3 py-1.5 font-medium text-xs rounded-t-lg transition-colors flex items-center gap-1.5 border-b-2 {activeTab ===
        'ports'
          ? 'border-emerald-400 text-emerald-400 bg-slate-800/50'
          : 'border-transparent text-slate-400 hover:text-slate-200 hover:bg-slate-900/50'}"
      >
        <span>DEV PORTS</span>
        {#if ports.length > 0}
          <span class="px-1.5 py-0.2 text-[9px] rounded-full bg-emerald-500/20 text-emerald-300 font-bold">
            {devPortsCount > 0 ? `${devPortsCount} Dev / ${ports.length}` : ports.length}
          </span>
        {/if}
      </button>

      <button
        onclick={() => (activeTab = 'processes')}
        class="px-3 py-1.5 font-medium text-xs rounded-t-lg transition-colors flex items-center gap-1.5 border-b-2 {activeTab ===
        'processes'
          ? 'border-cyan-400 text-cyan-400 bg-slate-800/50'
          : 'border-transparent text-slate-400 hover:text-slate-200 hover:bg-slate-900/50'}"
      >
        <span>PROCESS & LEAKS</span>
        <span class="px-1.5 py-0.2 text-[9px] rounded-full bg-cyan-500/20 text-cyan-300 font-bold">
          TOP 5
        </span>
      </button>
    </div>
  </header>

  <!-- Notification Banner / Error -->
  {#if isBrowserMode}
    <div class="bg-amber-500/10 border-b border-amber-500/30 px-3 py-1 text-amber-300 text-[10px] flex items-center justify-between">
      <span>Browser Preview Mode — Launch with <code class="font-bold bg-amber-500/20 px-1 rounded">npm run tauri dev</code> to access live system ports</span>
    </div>
  {/if}

  {#if errorMsg}
    <div class="bg-rose-500/10 border-b border-rose-500/30 px-3 py-1.5 text-rose-300 text-[10px] flex items-center justify-between">
      <span class="truncate">{errorMsg}</span>
      <button onclick={() => (errorMsg = null)} class="text-rose-400 hover:text-rose-200 font-bold ml-2">×</button>
    </div>
  {/if}

  <!-- Tab Contents -->
  <div class="flex-1 overflow-y-auto p-3 space-y-2">
    {#if activeTab === 'ports'}
      <!-- Search Filter Bar -->
      <div class="relative mb-2">
        <svg
          class="w-3.5 h-3.5 absolute left-2.5 top-2.5 text-slate-500"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
          />
        </svg>
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Filter by port (e.g. 8080) or process name (node, python)..."
          class="w-full bg-slate-900/90 border border-slate-800 rounded-lg pl-8 pr-7 py-1.5 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-emerald-500/50 transition-colors"
        />
        {#if searchQuery}
          <button
            onclick={() => (searchQuery = '')}
            class="absolute right-2.5 top-2.5 text-slate-500 hover:text-slate-300"
          >
            ×
          </button>
        {/if}
      </div>

      <!-- Ports List -->
      {#if filteredPorts.length === 0}
        <div class="py-12 text-center text-slate-500 flex flex-col items-center justify-center gap-2">
          <svg class="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
          <p class="text-xs font-medium">No listening sockets found</p>
          {#if searchQuery}
            <p class="text-[10px] text-slate-600">Try adjusting your search filter</p>
          {/if}
        </div>
      {:else}
        <div class="space-y-1.5">
          {#each filteredPorts as p (p.port + '_' + p.pid)}
            <div
              class="p-2 rounded-lg border transition-all duration-150 flex items-center justify-between gap-2 {p.is_dev_port
                ? 'bg-slate-900/80 border-emerald-500/30 hover:border-emerald-500/60 shadow-sm shadow-emerald-950/40'
                : 'bg-slate-900/40 border-slate-800/80 hover:border-slate-700'}"
            >
              <!-- Port Badge & Info -->
              <div class="flex items-center gap-2.5 min-w-0 flex-1">
                <!-- Port Pill -->
                <div
                  class="font-mono font-bold text-xs px-2 py-1 rounded-md shrink-0 flex items-center gap-1 {p.is_dev_port
                    ? 'bg-emerald-500/15 text-emerald-400 border border-emerald-500/30 shadow-[0_0_8px_rgba(16,185,129,0.15)]'
                    : 'bg-slate-800 text-slate-300 border border-slate-700'}"
                >
                  <span class="text-[10px] opacity-60">:</span>{p.port}
                </div>

                <!-- Process Details -->
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-1.5">
                    <span class="font-bold text-slate-200 truncate max-w-[130px]" title={p.process_name}>
                      {p.process_name}
                    </span>
                    <span class="text-[9px] px-1 py-0.2 rounded bg-slate-800 text-slate-400 font-mono">
                      {p.protocol}
                    </span>
                    {#if p.pid > 0}
                      <span class="text-[9px] text-slate-500 font-mono">PID {p.pid}</span>
                    {/if}
                  </div>

                  {#if p.exe_path}
                    <p class="text-[9px] text-slate-500 truncate max-w-[200px]" title={p.exe_path}>
                      {p.exe_path}
                    </p>
                  {/if}
                </div>
              </div>

              <!-- Quick Action Buttons -->
              <div class="flex items-center gap-1 shrink-0">
                <!-- Open in Browser -->
                <button
                  onclick={() => handleOpenBrowser(p.port)}
                  title="Open http://localhost:{p.port} in browser"
                  class="p-1.5 rounded text-slate-400 hover:text-cyan-400 hover:bg-cyan-500/10 transition-colors"
                >
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
                    />
                  </svg>
                </button>

                <!-- Copy URI -->
                <button
                  onclick={() => handleCopyUri(p.port)}
                  title="Copy URI (localhost:{p.port})"
                  class="p-1.5 rounded transition-colors {copiedPort === p.port
                    ? 'text-emerald-400 bg-emerald-500/10'
                    : 'text-slate-400 hover:text-emerald-400 hover:bg-emerald-500/10'}"
                >
                  {#if copiedPort === p.port}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                    </svg>
                  {:else}
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                      />
                    </svg>
                  {/if}
                </button>

                <!-- Kill Button -->
                {#if p.pid > 0}
                  <button
                    onclick={() => handleKill(p.pid)}
                    title={killConfirmPid === p.pid ? 'Click again to terminate process' : 'Terminate process'}
                    class="px-2 py-1 rounded text-[10px] font-bold transition-all duration-150 {killConfirmPid === p.pid
                      ? 'bg-rose-600 text-white animate-pulse shadow-md shadow-rose-950/60'
                      : 'bg-rose-500/10 text-rose-400 border border-rose-500/20 hover:bg-rose-500/20 hover:border-rose-500/40'}"
                  >
                    {killConfirmPid === p.pid ? 'Confirm?' : 'Kill'}
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {:else if activeTab === 'processes'}
      <!-- Process & Leaks (Top RAM processes) -->
      <div class="space-y-2">
        <div class="flex items-center justify-between text-[10px] text-slate-400 px-1 font-semibold">
          <span>TOP 5 MEMORY CONSUMERS</span>
          <span>RAM ALLOCATION</span>
        </div>

        {#if !stats || stats.top_memory_processes.length === 0}
          <div class="py-12 text-center text-slate-500">Scanning system processes...</div>
        {:else}
          <div class="space-y-1.5">
            {#each stats.top_memory_processes as proc, index (proc.pid)}
              <div class="bg-slate-900/70 border border-slate-800 rounded-lg p-2 flex flex-col gap-1.5">
                <div class="flex items-center justify-between gap-2">
                  <div class="flex items-center gap-2 min-w-0 flex-1">
                    <!-- Rank Badge -->
                    <span
                      class="w-4 h-4 rounded flex items-center justify-center font-bold text-[9px] shrink-0 {index === 0
                        ? 'bg-rose-500/20 text-rose-400 border border-rose-500/30'
                        : index === 1
                          ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                          : 'bg-slate-800 text-slate-400'}"
                    >
                      #{index + 1}
                    </span>

                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-1.5">
                        <span class="font-bold text-slate-200 truncate" title={proc.name}>
                          {proc.name}
                        </span>
                        <span class="text-[9px] text-slate-500 font-mono">PID {proc.pid}</span>
                      </div>
                    </div>
                  </div>

                  <!-- RAM Allocation & Kill -->
                  <div class="flex items-center gap-2 shrink-0">
                    <span class="font-bold font-mono text-cyan-400 text-xs">
                      {formatBytes(proc.memory_mb)}
                    </span>

                    <button
                      onclick={() => handleKill(proc.pid)}
                      title={killConfirmPid === proc.pid ? 'Click again to terminate process' : 'Terminate process'}
                      class="px-2 py-1 rounded text-[10px] font-bold transition-all duration-150 {killConfirmPid === proc.pid
                        ? 'bg-rose-600 text-white animate-pulse'
                        : 'bg-rose-500/10 text-rose-400 border border-rose-500/20 hover:bg-rose-500/20'}"
                    >
                      {killConfirmPid === proc.pid ? 'Confirm?' : 'Kill'}
                    </button>
                  </div>
                </div>

                <!-- Executable Path Subtitle -->
                {#if proc.exe_path}
                  <p class="text-[9px] text-slate-500 truncate px-1" title={proc.exe_path}>
                    {proc.exe_path}
                  </p>
                {/if}

                <!-- Memory Usage Relative Bar -->
                <div class="w-full bg-slate-800/60 rounded-full h-1 overflow-hidden">
                  <div
                    class="h-full bg-cyan-500/70 rounded-full"
                    style="width: {stats.memory_total_mb > 0
                      ? Math.min(100, (proc.memory_mb / (stats.top_memory_processes[0].memory_mb || 1)) * 100)
                      : 0}%"
                  ></div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Footer Info Bar -->
  <footer class="px-3 py-1.5 bg-slate-950 border-t border-slate-900 text-[9px] text-slate-500 flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
      <span>Port Sentinel v0.1.0</span>
    </div>
    <span class="text-slate-600">Press <kbd class="px-1 py-0.2 bg-slate-900 border border-slate-800 rounded text-slate-400">Esc</kbd> to hide</span>
  </footer>
</main>
