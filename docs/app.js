// zerowire documentation interactive logic

document.addEventListener('DOMContentLoaded', () => {
  // 1. Install Command Tabs
  const installTabs = document.querySelectorAll('.install-tab');
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
  const copyButtons = document.querySelectorAll('.copy-btn');
  const toast = document.getElementById('toast');

  function showToast(message) {
    if (!toast) return;
    toast.textContent = message;
    toast.classList.add('show');
    setTimeout(() => {
      toast.classList.remove('show');
    }, 2200);
  }

  copyButtons.forEach(btn => {
    btn.addEventListener('click', () => {
      const targetId = btn.dataset.copyTarget;
      let textToCopy = '';
      if (targetId) {
        const target = document.getElementById(targetId);
        textToCopy = target ? target.textContent.trim() : '';
      } else {
        const pre = btn.closest('.code-block')?.querySelector('.code-pre');
        textToCopy = pre ? pre.textContent.trim() : '';
      }

      if (textToCopy) {
        navigator.clipboard.writeText(textToCopy).then(() => {
          btn.classList.add('copied');
          const originalText = btn.innerHTML;
          btn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg> Copied`;
          showToast('Copied to clipboard');
          setTimeout(() => {
            btn.classList.remove('copied');
            btn.innerHTML = originalText;
          }, 2000);
        });
      }
    });
  });

  // 3. Benchmark Visualizer
  const benchTabs = document.querySelectorAll('.bench-tab');
  const benchBars = document.querySelectorAll('.bench-item');

  const benchmarkData = {
    throughput: [
      { name: 'zerowire Binary Validation', value: '169,000,000 ops/s', percent: 100, class: 'fill-cyan' },
      { name: 'zerowire validateAndView + Lazy Access', value: '6,880,000 ops/s', percent: 4.1, class: 'fill-purple' },
      { name: 'JSON.parse() Alone', value: '1,600,000 ops/s', percent: 1.0, class: 'fill-gray' },
      { name: 'JSON.parse() + Zod Validation', value: '460,000 ops/s', percent: 0.3, class: 'fill-gray' }
    ],
    latency: [
      { name: 'zerowire Binary Validation', value: '5.92 ns', percent: 100, class: 'fill-cyan' }, // 100% of efficiency
      { name: 'zerowire validateAndView + Lazy Access', value: '145.16 ns', percent: 40, class: 'fill-purple' },
      { name: 'JSON.parse() Alone', value: '624.59 ns', percent: 15, class: 'fill-gray' },
      { name: 'JSON.parse() + Zod Validation', value: '2,170.00 ns', percent: 3, class: 'fill-gray' }
    ],
    memory: [
      { name: 'zerowire Binary Validation', value: '0 B / op', percent: 100, class: 'fill-cyan' },
      { name: 'zerowire validateAndView + Lazy Access', value: '229 B / op', percent: 30, class: 'fill-purple' },
      { name: 'JSON.parse() Alone', value: '73 B / op', percent: 60, class: 'fill-gray' },
      { name: 'JSON.parse() + Zod Validation', value: '2,010 B / op', percent: 4, class: 'fill-gray' }
    ]
  };

  function updateBenchmarkView(metric) {
    const data = benchmarkData[metric];
    if (!data) return;

    benchBars.forEach((item, index) => {
      const entry = data[index];
      if (!entry) return;

      const valEl = item.querySelector('.bench-value');
      const fillEl = item.querySelector('.bench-bar-fill');

      if (valEl) valEl.textContent = entry.value;
      if (fillEl) {
        fillEl.className = `bench-bar-fill ${entry.class}`;
        fillEl.style.width = `${Math.max(entry.percent, 3)}%`;
      }
    });
  }

  benchTabs.forEach(tab => {
    tab.addEventListener('click', () => {
      benchTabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');
      const metric = tab.dataset.metric;
      updateBenchmarkView(metric);
    });
  });

  // 4. Interactive Struct Layout Visualizer
  const presets = {
    TradeExecution: {
      code: `type u32 = number & { __brand: "u32" };\n\nexport interface TradeExecution {\n  timestamp: number;     // 8B f64\n  tradeId: u32;          // 4B u32\n  side: "BUY" | "SELL";  // 1B enum\n  isMaker: boolean;      // 1B boolean\n  symbol: string;        // 8B (u32 off, u32 len)\n  fills: number[];       // 8B (u32 off, u32 len)\n}`,
      headerSize: 32,
      alignment: 8,
      padding: '2 Bytes (aligned to 8B)',
      rows: [
        { offset: '[0x00..0x07]', field: 'timestamp', type: 'f64 (IEEE-754)', size: '8B' },
        { offset: '[0x08..0x0B]', field: 'tradeId', type: 'u32 (unsigned)', size: '4B' },
        { offset: '[0x0C..0x13]', field: 'symbol', type: 'string descriptor', size: '8B' },
        { offset: '[0x14..0x1B]', field: 'fills', type: 'array descriptor', size: '8B' },
        { offset: '[0x1C]', field: 'side', type: 'enum (2 variants)', size: '1B' },
        { offset: '[0x1D]', field: 'isMaker', type: 'boolean', size: '1B' },
        { offset: '[0x1E..0x1F]', field: '<padding>', type: 'zero-filled alignment pad', size: '2B' }
      ]
    },
    MarketTick: {
      code: `type u32 = number & { __brand: "u32" };\n\nexport interface MarketTick {\n  price: number;              // 8B f64\n  volume: u32;                // 4B u32\n  exchange: "NYSE"|"NASDAQ"|"CME"; // 1B enum\n  symbol: string;             // 8B (u32 off, u32 len)\n}`,
      headerSize: 24,
      alignment: 8,
      padding: '3 Bytes (aligned to 8B)',
      rows: [
        { offset: '[0x00..0x07]', field: 'price', type: 'f64 (IEEE-754)', size: '8B' },
        { offset: '[0x08..0x0B]', field: 'volume', type: 'u32 (unsigned)', size: '4B' },
        { offset: '[0x0C..0x13]', field: 'symbol', type: 'string descriptor', size: '8B' },
        { offset: '[0x14]', field: 'exchange', type: 'enum (3 variants)', size: '1B' },
        { offset: '[0x15..0x17]', field: '<padding>', type: 'zero-filled alignment pad', size: '3B' }
      ]
    },
    SensorTelemetry: {
      code: `type u32 = number & { __brand: "u32" };\ntype f32 = number & { __brand: "f32" };\n\nexport interface SensorTelemetry {\n  deviceId: u32;    // 4B\n  voltage: f32;     // 4B\n  current: f32;     // 4B\n  temperature: f32; // 4B\n  location: string; // 8B (u32 off, u32 len)\n}`,
      headerSize: 24,
      alignment: 4,
      padding: '0 Bytes (perfect packing)',
      rows: [
        { offset: '[0x00..0x03]', field: 'deviceId', type: 'u32 (unsigned)', size: '4B' },
        { offset: '[0x04..0x07]', field: 'voltage', type: 'f32 (single precision)', size: '4B' },
        { offset: '[0x08..0x0B]', field: 'current', type: 'f32 (single precision)', size: '4B' },
        { offset: '[0x0C..0x0F]', field: 'temperature', type: 'f32 (single precision)', size: '4B' },
        { offset: '[0x10..0x17]', field: 'location', type: 'string descriptor', size: '8B' }
      ]
    }
  };

  const presetButtons = document.querySelectorAll('.preset-btn');
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
          <td class="offset-col">${r.offset}</td>
          <td class="field-col">${r.field}</td>
          <td class="type-col">${r.type}</td>
          <td class="size-col">${r.size}</td>
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

  // Render initial preset
  renderPreset('TradeExecution');

  // 5. Search Modal Logic
  const searchTrigger = document.querySelector('.search-trigger');
  const searchModal = document.getElementById('search-modal');
  const searchInput = document.getElementById('search-input');
  const searchResults = document.getElementById('search-results');

  const searchIndex = [
    { title: 'Overview', desc: 'Zero-parse AOT TypeScript validator and binary codec architecture', href: '#overview' },
    { title: 'Benchmarks', desc: 'Throughput and latency comparison vs Zod and JSON.parse', href: '#benchmarks' },
    { title: 'Core Features', desc: 'Zero tsc dependency, branchless execution, struct packing', href: '#features' },
    { title: 'Interactive Layout Visualizer', desc: 'Explore memory layouts, offsets, and packing diagrams', href: '#visualizer' },
    { title: 'Quick Start', desc: 'Define TypeScript schemas and compile wire codecs', href: '#quickstart' },
    { title: 'Lazy Flyweight Views', desc: 'Zero-copy field access directly from raw Uint8Array buffers', href: '#flyweight' },
    { title: 'Binary Encoding', desc: 'Single-pass serialization directly into typed binary buffers', href: '#encoding' },
    { title: 'Minimal Perfect Hashing', desc: 'O(1) variant and tagged union dispatch without string comparisons', href: '#mphf' },
    { title: 'Bundler Plugins', desc: 'Vite, Rolldown, Rollup, and esbuild integration', href: '#bundlers' },
    { title: 'WASM SIMD Scanner', desc: '128-bit SIMD structural token scanner with prefix-XOR masking', href: '#simd' }
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
      searchResults.innerHTML = `<li style="padding: 1.5rem; text-align: center; color: var(--text-muted); font-size: 0.9rem;">No results found for "${query}"</li>`;
      return;
    }

    searchResults.innerHTML = filtered.map(item => `
      <li class="search-result-item">
        <a href="${item.href}">
          <div class="search-result-title">${item.title}</div>
          <div class="search-result-desc">${item.desc}</div>
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

  // 6. ScrollSpy for Sidebar Active State
  const sections = document.querySelectorAll('.doc-section');
  const sidebarLinks = document.querySelectorAll('.sidebar-item a');

  window.addEventListener('scroll', () => {
    let currentId = '';
    sections.forEach(section => {
      const top = section.offsetTop - 120;
      if (window.scrollY >= top) {
        currentId = section.getAttribute('id');
      }
    });

    if (currentId) {
      sidebarLinks.forEach(link => {
        const item = link.closest('.sidebar-item');
        if (link.getAttribute('href') === `#${currentId}`) {
          item?.classList.add('active');
        } else {
          item?.classList.remove('active');
        }
      });
    }
  });
});
