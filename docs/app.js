// zerowire documentation interactive logic

document.addEventListener('DOMContentLoaded', () => {
  // 1. Install Command Tabs
  const installTabs = document.querySelectorAll('.install-tab-btn');
  const installCmd = document.getElementById('install-cmd-text');
  
  const commands = {
    pnpm: 'pnpm add @zerowire/runtime && pnpm add -D @zerowire/unplugin',
    npm: 'npm install @zerowire/runtime && npm install -D @zerowire/unplugin',
    yarn: 'yarn add @zerowire/runtime && yarn add -D @zerowire/unplugin',
    bun: 'bun add @zerowire/runtime && bun add -d @zerowire/unplugin'
  };

  installTabs.forEach(tab => {
    tab.addEventListener('click', () => {
      installTabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');
      const pm = tab.dataset.pm;
      if (commands[pm] && installCmd) {
        installCmd.textContent = commands[pm];
      }
    });
  });

  // 2. Copy Buttons & Toast
  const copyButtons = document.querySelectorAll('.btn-copy');
  const toast = document.getElementById('toast');

  function showToast(message) {
    if (!toast) return;
    toast.textContent = message;
    toast.classList.add('show');
    setTimeout(() => {
      toast.classList.remove('show');
    }, 2000);
  }

  copyButtons.forEach(btn => {
    btn.addEventListener('click', () => {
      const targetId = btn.dataset.copyTarget;
      let textToCopy = '';
      if (targetId) {
        const target = document.getElementById(targetId);
        textToCopy = target ? target.textContent.trim() : '';
      } else {
        const pre = btn.closest('.code-panel')?.querySelector('.code-content');
        textToCopy = pre ? pre.textContent.trim() : '';
      }

      if (textToCopy) {
        navigator.clipboard.writeText(textToCopy).then(() => {
          btn.classList.add('copied');
          const originalText = btn.innerHTML;
          btn.innerHTML = `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg> Copied`;
          showToast('Copied to clipboard');
          setTimeout(() => {
            btn.classList.remove('copied');
            btn.innerHTML = originalText;
          }, 1800);
        });
      }
    });
  });

  // 3. Benchmark Visualizer
  const benchTabs = document.querySelectorAll('.bench-nav-btn');
  const benchRows = document.querySelectorAll('.bench-row');

  const benchmarkData = {
    throughput: [
      { name: 'zerowire Binary Validation', value: '169.0M ops/s', percent: 100, isWinner: true },
      { name: 'zerowire validateAndView + Lazy Access', value: '6.88M ops/s', percent: 28, isWinner: false },
      { name: 'JSON.parse() Alone', value: '1.60M ops/s', percent: 14, isWinner: false },
      { name: 'JSON.parse() + Zod Validation', value: '0.46M ops/s', percent: 5, isWinner: false }
    ],
    latency: [
      { name: 'zerowire Binary Validation', value: '5.92 ns', percent: 100, isWinner: true },
      { name: 'zerowire validateAndView + Lazy Access', value: '145.16 ns', percent: 36, isWinner: false },
      { name: 'JSON.parse() Alone', value: '624.59 ns', percent: 18, isWinner: false },
      { name: 'JSON.parse() + Zod Validation', value: '2,170.00 ns', percent: 6, isWinner: false }
    ],
    memory: [
      { name: 'zerowire Binary Validation', value: '0 B / op', percent: 100, isWinner: true },
      { name: 'zerowire validateAndView + Lazy Access', value: '229 B / op', percent: 35, isWinner: false },
      { name: 'JSON.parse() Alone', value: '73 B / op', percent: 62, isWinner: false },
      { name: 'JSON.parse() + Zod Validation', value: '2,010 B / op', percent: 8, isWinner: false }
    ]
  };

  function updateBenchmarkView(metric) {
    const data = benchmarkData[metric];
    if (!data) return;

    benchRows.forEach((row, index) => {
      const entry = data[index];
      if (!entry) return;

      const scoreEl = row.querySelector('.bench-score');
      const barEl = row.querySelector('.bench-bar');

      if (scoreEl) scoreEl.textContent = entry.value;
      if (barEl) {
        barEl.className = `bench-bar ${entry.isWinner ? 'bar-winner' : 'bar-default'}`;
        barEl.style.width = `${Math.max(entry.percent, 3)}%`;
      }
    });
  }

  benchTabs.forEach(tab => {
    tab.addEventListener('click', () => {
      benchTabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');
      updateBenchmarkView(tab.dataset.metric);
    });
  });

  // 4. Interactive Struct Layout Visualizer
  const presets = {
    TradeExecution: {
      code: `type u32 = number & { __brand: "u32" };\n\nexport interface TradeExecution {\n  timestamp: number;     // 8B f64 (descending alignment)\n  tradeId: u32;          // 4B u32\n  symbol: string;        // 8B (u32 offset, u32 len)\n  fills: number[];       // 8B (u32 offset, u32 len)\n  side: "BUY" | "SELL";  // 1B enum (u8)\n  isMaker: boolean;      // 1B boolean (u8)\n}`,
      headerSize: 32,
      alignment: 8,
      padding: '2 Bytes (aligned to 8B)',
      rows: [
        { offset: '[0x00..0x07]', field: 'timestamp', type: 'f64 (IEEE-754)', size: '8B' },
        { offset: '[0x08..0x0B]', field: 'tradeId', type: 'u32 (unsigned int)', size: '4B' },
        { offset: '[0x0C..0x13]', field: 'symbol', type: 'string descriptor', size: '8B' },
        { offset: '[0x14..0x1B]', field: 'fills', type: 'f64[] descriptor', size: '8B' },
        { offset: '[0x1C]', field: 'side', type: 'enum (2 variants)', size: '1B' },
        { offset: '[0x1D]', field: 'isMaker', type: 'boolean', size: '1B' },
        { offset: '[0x1E..0x1F]', field: '<padding>', type: 'alignment pad', size: '2B' }
      ]
    },
    MarketTick: {
      code: `type u32 = number & { __brand: "u32" };\n\nexport interface MarketTick {\n  price: number;              // 8B f64\n  volume: u32;                // 4B u32\n  symbol: string;             // 8B (u32 offset, u32 len)\n  exchange: "NYSE"|"NASDAQ"|"CME"; // 1B enum\n}`,
      headerSize: 24,
      alignment: 8,
      padding: '3 Bytes (aligned to 8B)',
      rows: [
        { offset: '[0x00..0x07]', field: 'price', type: 'f64 (IEEE-754)', size: '8B' },
        { offset: '[0x08..0x0B]', field: 'volume', type: 'u32 (unsigned int)', size: '4B' },
        { offset: '[0x0C..0x13]', field: 'symbol', type: 'string descriptor', size: '8B' },
        { offset: '[0x14]', field: 'exchange', type: 'enum (3 variants)', size: '1B' },
        { offset: '[0x15..0x17]', field: '<padding>', type: 'alignment pad', size: '3B' }
      ]
    },
    SensorTelemetry: {
      code: `type u32 = number & { __brand: "u32" };\ntype f32 = number & { __brand: "f32" };\n\nexport interface SensorTelemetry {\n  deviceId: u32;    // 4B\n  voltage: f32;     // 4B\n  current: f32;     // 4B\n  temperature: f32; // 4B\n  location: string; // 8B (u32 offset, u32 len)\n}`,
      headerSize: 24,
      alignment: 4,
      padding: '0 Bytes (perfect packing)',
      rows: [
        { offset: '[0x00..0x03]', field: 'deviceId', type: 'u32 (unsigned int)', size: '4B' },
        { offset: '[0x04..0x07]', field: 'voltage', type: 'f32 (single precision)', size: '4B' },
        { offset: '[0x08..0x0B]', field: 'current', type: 'f32 (single precision)', size: '4B' },
        { offset: '[0x0C..0x0F]', field: 'temperature', type: 'f32 (single precision)', size: '4B' },
        { offset: '[0x10..0x17]', field: 'location', type: 'string descriptor', size: '8B' }
      ]
    }
  };

  const presetButtons = document.querySelectorAll('.preset-chip');
  const schemaCodeEl = document.getElementById('visualizer-schema-code');
  const headerSizeEl = document.getElementById('stat-header-size');
  const alignmentEl = document.getElementById('stat-alignment');
  const paddingEl = document.getElementById('stat-padding');
  const tableBody = document.getElementById('layout-table-body');

  function renderPreset(name) {
    const data = presets[name];
    if (!data) return;

    if (schemaCodeEl) schemaCodeEl.textContent = data.code;
    if (headerSizeEl) headerSizeEl.textContent = `${data.headerSize} Bytes`;
    if (alignmentEl) alignmentEl.textContent = `${data.alignment} Bytes`;
    if (paddingEl) paddingEl.textContent = data.padding;

    if (tableBody) {
      tableBody.innerHTML = data.rows.map(r => `
        <tr>
          <td class="cell-offset">${r.offset}</td>
          <td class="cell-field">${r.field}</td>
          <td class="cell-type">${r.type}</td>
          <td class="cell-size">${r.size}</td>
        </tr>
      `).join('');
    }
  }

  presetButtons.forEach(btn => {
    btn.addEventListener('click', () => {
      presetButtons.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      renderPreset(btn.dataset.preset);
    });
  });

  renderPreset('TradeExecution');

  // 5. Search Modal Logic
  const searchTrigger = document.getElementById('btn-search-trigger');
  const searchModal = document.getElementById('search-modal');
  const searchInput = document.getElementById('search-input');
  const searchResults = document.getElementById('search-results');

  const searchIndex = [
    { title: 'Overview', desc: 'AOT binary codec and branchless validator architecture', href: '#overview' },
    { title: 'Benchmarks', desc: 'Throughput and latency comparison vs Zod and JSON.parse', href: '#benchmarks' },
    { title: 'Core Features', desc: 'Zero tsc dependency, branchless execution, struct packing', href: '#features' },
    { title: 'Quick Start', desc: 'Declaring TypeScript schemas and compiling wire codecs', href: '#quickstart' },
    { title: 'HTTP and Web Framework Guide', desc: 'Validating raw request arrayBuffer slices in Hono and Fastify', href: '#http-guide' },
    { title: 'Worker Thread Guide', desc: 'Zero-copy messaging across threads with SharedArrayBuffer', href: '#worker-guide' },
    { title: 'Memory Layout Visualizer', desc: 'Interactive struct packing and alignment calculation', href: '#visualizer' },
    { title: 'Flyweight Views', desc: 'Lazy accessor memoization and unrolled short string decoding', href: '#flyweight' },
    { title: 'Binary Encoding', desc: 'Single-pass serialization directly into typed binary buffers', href: '#encoding' },
    { title: 'Minimal Perfect Hashing', desc: 'O(1) variant and tagged union dispatch without string lookups', href: '#mphf' },
    { title: 'Bundler Setup', desc: 'Vite, Rolldown, Rollup, and esbuild integration', href: '#bundlers' },
    { title: 'WASM SIMD Tokenizer', desc: '128-bit SIMD structural token scanner with prefix-XOR masking', href: '#simd' }
  ];

  function openSearch() {
    if (searchModal) {
      searchModal.classList.add('open');
      searchInput?.focus();
      renderSearchResults('');
    }
  }

  function closeSearch() {
    if (searchModal) {
      searchModal.classList.remove('open');
      if (searchInput) searchInput.value = '';
    }
  }

  function renderSearchResults(query) {
    if (!searchResults) return;
    const q = query.toLowerCase().trim();
    const filtered = searchIndex.filter(item => 
      item.title.toLowerCase().includes(q) || item.desc.toLowerCase().includes(q)
    );

    if (filtered.length === 0) {
      searchResults.innerHTML = `<li style="padding: 1rem; text-align: center; color: var(--text-subtle); font-size: 0.82rem;">No results found for "${query}"</li>`;
      return;
    }

    searchResults.innerHTML = filtered.map(item => `
      <li class="modal-item">
        <a href="${item.href}">
          <div class="modal-item-title">${item.title}</div>
          <div class="modal-item-desc">${item.desc}</div>
        </a>
      </li>
    `).join('');

    searchResults.querySelectorAll('a').forEach(a => {
      a.addEventListener('click', () => {
        closeSearch();
      });
    });
  }

  if (searchTrigger) {
    searchTrigger.addEventListener('click', openSearch);
  }

  if (searchModal) {
    searchModal.addEventListener('click', (e) => {
      if (e.target === searchModal) closeSearch();
    });
  }

  searchInput?.addEventListener('input', (e) => {
    renderSearchResults(e.target.value);
  });

  window.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      openSearch();
    }
    if (e.key === 'Escape') {
      closeSearch();
    }
  });

  // 6. ScrollSpy
  const sections = document.querySelectorAll('.doc-section');
  const sidebarLinks = document.querySelectorAll('.sidebar-link a');

  window.addEventListener('scroll', () => {
    let currentId = '';
    sections.forEach(section => {
      const top = section.offsetTop - 100;
      if (window.scrollY >= top) {
        currentId = section.getAttribute('id');
      }
    });

    if (currentId) {
      sidebarLinks.forEach(link => {
        const item = link.closest('.sidebar-link');
        if (link.getAttribute('href') === `#${currentId}`) {
          item?.classList.add('active');
        } else {
          item?.classList.remove('active');
        }
      });
    }
  });
});
