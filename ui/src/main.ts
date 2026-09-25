import Graph from "graphology";
import Sigma from "sigma";
import forceAtlas2 from "graphology-layout-forceatlas2";
import FA2Layout from "graphology-layout-forceatlas2/worker";

type Sym = [string, string, number, number, number]; // name, kind, start, end, callers
interface NodeData {
  p: string;
  c: number;
  r: number;
  l: number;
  g: string;
  t: boolean;
  s: Sym[];
  /** db mode: code that queries the table [file, line, symbol, via] */
  q?: [string, number, string | null, string][];
  /** db mode: [file, line] of the definition */
  src?: [string, number];
  /** db mode: indexes [name, columns, unique] */
  ix?: [string, string[], boolean][];
}
interface Data {
  mode?: "code" | "db";
  origin?: string;
  version: string;
  repo: string;
  commit?: string | null;
  branch?: string | null;
  web?: string | null;
  root?: string;
  stats: { files: number; code_files: number; symbols: number; call_edges: number; file_edges: number; index_ms: number };
  communities: { id: number; name: string; size: number; kind?: string }[];
  nodes: NodeData[];
  edges: [number, number, number, number][];
}

declare global {
  interface Window {
    __REPOMAP__?: Data;
  }
}

const LIVE = !window.__REPOMAP__;
const PALETTE = [
  "#8aa4ff", "#ff7eb6", "#42d6a4", "#ffb454", "#b18cff", "#4dd0e1", "#ff8a65", "#a5d86b",
  "#ffd54f", "#64b5f6", "#e57373", "#81c784", "#ce7de0", "#4db6ac", "#f4a261", "#90caf9",
  "#c5e1a5", "#b39ddb", "#ffcc80", "#80deea", "#f48fb1", "#aed581", "#9fa8da", "#ffe082",
];
const OTHER = "#59627a";
const DIM = "#171b25";
const DEPTH = ["#ffffff", "#ff5d5d", "#ff9d42", "#ffd166"];
const DEP = "#36d6e7";

const $ = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector(sel) as T;
const esc = (s: string) => s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
const fmt = (n: number) => n.toLocaleString("en-US");
const base = (p: string) => p.slice(p.lastIndexOf("/") + 1);
const dir = (p: string) => (p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : "");

const AUX: Record<string, string> = { tests: "#5d6679", examples: "#7b6f5c", docs: "#5c7466", benchmarks: "#6c5f7a" };
const KIND = new Map<number, string>();

function commColor(c: number) {
  const k = KIND.get(c);
  if (k && k !== "core") return AUX[k] ?? OTHER;
  return c < PALETTE.length ? PALETTE[c] : OTHER;
}

function rng(seed: number) {
  return () => {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

async function load(): Promise<Data> {
  if (window.__REPOMAP__) return window.__REPOMAP__;
  const r = await fetch("api/graph");
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

function mount(data: Data) {
  const n = data.nodes.length;
  const DB = data.mode === "db";
  for (const c of data.communities) KIND.set(c.id, c.kind ?? "core");
  const isAux = (d: NodeData) => d.t || (KIND.get(d.c) ?? "core") !== "core";
  const coreComms = data.communities.filter((c) => (c.kind ?? "core") === "core");
  const auxComms = data.communities.filter((c) => (c.kind ?? "core") !== "core");
  const graph = new Graph({ type: "directed", multi: false, allowSelfLoops: false });
  const rand = rng(42);

  // Seed positions: communities on a golden-angle spiral, files in a disk around each.
  const centers = new Map<number, [number, number]>();
  let acc = 0;
  data.communities.forEach((c, i) => {
    const r = Math.sqrt(acc + c.size / 2) * 12;
    const a = i * 2.39996;
    centers.set(c.id, [Math.cos(a) * r, Math.sin(a) * r]);
    acc += c.size;
  });
  const commSize = new Map(data.communities.map((c) => [c.id, c.size]));
  data.nodes.forEach((d, i) => {
    const [cx, cy] = centers.get(d.c) ?? [0, 0];
    const spread = Math.sqrt(commSize.get(d.c) ?? 1) * 4 + 2;
    const a = rand() * Math.PI * 2;
    const rr = Math.sqrt(rand()) * spread;
    graph.addNode(String(i), {
      x: cx + Math.cos(a) * rr,
      y: cy + Math.sin(a) * rr,
      size: 1.8 + 12 * Math.sqrt(d.r) + Math.min(2.5, Math.log10(1 + d.l) * 0.5),
      color: commColor(d.c),
      label: base(d.p),
      comm: d.c,
    });
  });
  for (const [a, b, imp, calls] of data.edges) {
    const key = `${a}>${b}`;
    if (graph.hasEdge(key)) continue;
    const w = imp + 0.5 * calls;
    const aux = isAux(data.nodes[a]) || isAux(data.nodes[b]);
    const same = !aux && data.nodes[a].c === data.nodes[b].c;
    graph.addDirectedEdgeWithKey(key, String(a), String(b), {
      // Layout weight: pull modules together, let modules drift apart.
      weight: aux ? w * 0.5 : same ? w * 4 : w * 0.25,
      size: Math.min(1.6, 0.25 + Math.log1p(w) * 0.25),
      color: mix(commColor(data.nodes[a].c), same ? 0.3 : 0.1),
    });
  }

  // ---- state
  const state = {
    hovered: null as string | null,
    neighbors: new Set<string>(),
    selected: null as string | null,
    mode: "none" as "none" | "impact" | "deps",
    depth: new Map<string, number>(),
    focusComm: null as number | null,
    hiddenComms: new Set<number>(),
    edges: data.edges.length < 60000,
    // Big repos open on the core modules; tests/examples/docs are one click away.
    tests: data.nodes.filter((d) => !isAux(d)).length < 40,
    labels: true,
  };

  const container = $("#stage");
  const renderer = new Sigma(graph, container, {
    renderEdgeLabels: false,
    hideEdgesOnMove: data.edges.length > 20000,
    labelFont: getComputedStyle(document.body).fontFamily,
    labelSize: 12,
    labelWeight: "500",
    labelColor: { color: "#c9d1e3" },
    labelDensity: 0.6,
    labelGridCellSize: 110,
    labelRenderedSizeThreshold: n > 3000 ? 9 : 6,
    zIndex: true,
    minCameraRatio: 0.02,
    maxCameraRatio: 4,
    stagePadding: 40,
    defaultEdgeType: "line",
    defaultDrawNodeLabel: drawLabel,
    defaultDrawNodeHover: drawHover,
    nodeReducer: (node, attrs) => {
      const d = data.nodes[+node];
      const res: Record<string, unknown> = { ...attrs };
      if ((!state.tests && isAux(d)) || state.hiddenComms.has(d.c)) {
        res.hidden = true;
        return res;
      }
      if (!state.labels) res.label = "";
      if (state.mode !== "none") {
        const dd = state.depth.get(node);
        if (dd === undefined) {
          res.color = DIM;
          res.label = "";
          res.zIndex = 0;
        } else {
          res.color = state.mode === "deps" ? (dd === 0 ? "#ffffff" : DEP) : DEPTH[Math.min(dd, 3)];
          res.zIndex = 2;
          res.size = (attrs.size as number) * (dd === 0 ? 1.6 : 1.25);
          if (dd === 0 || (dd === 1 && d.r > 0.15)) res.forceLabel = true;
        }
      } else if (state.hovered) {
        if (node === state.hovered || state.neighbors.has(node)) {
          res.zIndex = 2;
          res.forceLabel = node === state.hovered;
        } else {
          res.color = DIM;
          res.label = "";
          res.zIndex = 0;
        }
      } else if (state.focusComm !== null && d.c !== state.focusComm) {
        res.color = DIM;
        res.label = "";
        res.zIndex = 0;
      }
      if (node === state.selected) {
        res.highlighted = true;
        res.forceLabel = true;
        res.zIndex = 3;
      }
      return res;
    },
    edgeReducer: (edge, attrs) => {
      const res: Record<string, unknown> = { ...attrs };
      const [s, t] = graph.extremities(edge);
      const ds = data.nodes[+s], dt = data.nodes[+t];
      if ((!state.tests && (isAux(ds) || isAux(dt))) || state.hiddenComms.has(ds.c) || state.hiddenComms.has(dt.c)) {
        res.hidden = true;
        return res;
      }
      if (state.mode !== "none") {
        const a = state.depth.get(s), b = state.depth.get(t);
        const tree = a !== undefined && b !== undefined && (state.mode === "impact" ? a === b + 1 : b === a + 1);
        if (!tree) res.hidden = true;
        else {
          const d = (state.mode === "impact" ? a : b)!;
          res.color = mix(state.mode === "deps" ? DEP : DEPTH[Math.min(Math.max(d, 1), 3)], 0.6);
          res.size = 1.2;
          res.zIndex = 1;
        }
      } else if (state.hovered) {
        if (s === state.hovered || t === state.hovered) {
          res.color = mix(s === state.hovered ? "#8aa4ff" : "#ff7eb6", 0.85);
          res.size = 1.3;
          res.zIndex = 1;
        } else res.hidden = true;
      } else if (state.focusComm !== null) {
        if (ds.c !== state.focusComm && dt.c !== state.focusComm) res.hidden = true;
        else res.color = mix(commColor(state.focusComm), 0.4);
      } else if (!state.edges) {
        res.hidden = true;
      }
      return res;
    },
  });

  // ---- layout
  const inferred = forceAtlas2.inferSettings(graph);
  const layout = new FA2Layout(graph, {
    settings: {
      ...inferred,
      barnesHutOptimize: n > 800,
      barnesHutTheta: 0.6,
      linLogMode: true,
      outboundAttractionDistribution: false,
      gravity: 0.25,
      scalingRatio: n > 3000 ? 12 : 8,
      slowDown: 1 + Math.log10(n + 1),
      edgeWeightInfluence: 1,
    },
    getEdgeWeight: "weight",
  });
  let layoutTimer = 0;
  const runLayout = (ms: number) => {
    layout.start();
    setBtn("layout", true);
    clearTimeout(layoutTimer);
    layoutTimer = window.setTimeout(stopLayout, ms);
  };
  const stopLayout = () => {
    layout.stop();
    setBtn("layout", false);
    document.body.dataset.ready = "1";
  };
  runLayout(Math.min(20000, 3500 + n * 1.2));

  // ---- header
  const modRow = (c: { id: number; name: string; size: number }) =>
    `<div class="mod" data-c="${c.id}" title="${esc(c.name)}"><span class="dot" style="color:${commColor(c.id)};background:${commColor(c.id)}"></span><span class="name">${esc(c.name)}</span><span class="count">${c.size}</span></div>`;
  const legendHTML =
    coreComms.slice(0, 60).map(modRow).join("") +
    (auxComms.length ? `<div class="mod-sep">Tests, examples &amp; docs</div>` + auxComms.map(modRow).join("") : "");
  $("#brand .title").innerHTML = `repomap <span>/ ${esc(data.repo)}</span>`;
  $("#brand .stats").textContent = DB
    ? `${fmt(n)} tables · ${fmt(data.stats.symbols)} columns · ${fmt(data.stats.file_edges)} foreign keys · ${fmt(data.stats.call_edges)} code refs`
    : `${fmt(data.stats.code_files)} files · ${fmt(data.stats.symbols)} symbols · ${fmt(data.stats.call_edges)} calls · ${coreComms.length} modules`;
  if (DB) {
    (document.querySelector('#controls [data-c="tests"]') as HTMLElement | null)?.style.setProperty("display", "none");
    $("#brand .title").innerHTML = `repomap db <span>/ ${esc(data.repo)}</span>`;
    $("#legend h3").textContent = "Table groups";
    $<HTMLInputElement>("#search input").placeholder = "Search tables and columns…";
  }
  $("#legend .list").innerHTML = legendHTML;
  $("#legend .list").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest(".mod") as HTMLElement | null;
    if (!el) return;
    const c = +el.dataset.c!;
    if ((e as MouseEvent).shiftKey || (e as MouseEvent).altKey) {
      state.hiddenComms.has(c) ? state.hiddenComms.delete(c) : state.hiddenComms.add(c);
      el.classList.toggle("off", state.hiddenComms.has(c));
    } else {
      state.focusComm = state.focusComm === c ? null : c;
      document.querySelectorAll(".mod").forEach((m) => m.classList.toggle("on", +(m as HTMLElement).dataset.c! === state.focusComm));
      if (state.focusComm !== null) zoomTo(graph.filterNodes((_, a) => a.comm === c));
      else renderer.getCamera().animatedReset({ duration: 500 });
    }
    renderer.refresh();
  });
  $("#legend header button").addEventListener("click", () => {
    state.focusComm = null;
    state.hiddenComms.clear();
    document.querySelectorAll(".mod").forEach((m) => m.classList.remove("on", "off"));
    renderer.getCamera().animatedReset({ duration: 500 });
    renderer.refresh();
  });

  // ---- hover tooltip
  const tip = $("#tip");
  renderer.on("enterNode", ({ node, event }) => {
    state.hovered = node;
    state.neighbors = new Set(graph.neighbors(node));
    const d = data.nodes[+node];
    tip.innerHTML = DB
      ? `<b>${esc(d.p)}</b><span>${fmt(d.l)} columns · referenced by ${graph.inDegree(node)} · references ${graph.outDegree(node)} · ${(d.q ?? []).length} code refs</span>`
      : `<b>${esc(base(d.p))}</b><span>${esc(d.p)}</span><br><span>${fmt(d.l)} lines · ${d.s.length} symbols · ${graph.inDegree(node)} in / ${graph.outDegree(node)} out</span>`;
    tip.style.display = "block";
    moveTip(event.x, event.y);
    renderer.refresh({ skipIndexation: true });
    container.style.cursor = "pointer";
  });
  renderer.on("leaveNode", () => {
    state.hovered = null;
    state.neighbors.clear();
    tip.style.display = "none";
    renderer.refresh({ skipIndexation: true });
    container.style.cursor = "";
  });
  renderer.getMouseCaptor().on("mousemovebody", (e) => {
    if (state.hovered) moveTip(e.x, e.y);
  });
  function moveTip(x: number, y: number) {
    const r = container.getBoundingClientRect();
    tip.style.left = Math.min(r.left + x + 16, window.innerWidth - 440) + "px";
    tip.style.top = r.top + y + 16 + "px";
  }
  renderer.on("clickNode", ({ node }) => select(node, true));
  renderer.on("clickStage", () => clearSelection());

  // ---- camera helpers
  function zoomTo(nodes: string[]) {
    if (!nodes.length) return;
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const nd of nodes) {
      const p = renderer.getNodeDisplayData(nd);
      if (!p) continue;
      x0 = Math.min(x0, p.x); y0 = Math.min(y0, p.y); x1 = Math.max(x1, p.x); y1 = Math.max(y1, p.y);
    }
    const ratio = Math.min(1.2, Math.max(0.06, Math.max(x1 - x0, y1 - y0) * 1.4));
    renderer.getCamera().animate({ x: (x0 + x1) / 2, y: (y0 + y1) / 2, ratio }, { duration: 600 });
  }

  // ---- selection & panel
  const panel = $("#panel");
  function clearSelection() {
    state.selected = null;
    state.mode = "none";
    state.depth.clear();
    panel.classList.remove("open");
    document.body.classList.remove("panel-open");
    history.replaceState(null, "", location.pathname + location.search);
    renderer.refresh({ skipIndexation: true });
  }

  function select(node: string, keepCamera = false, symLine?: number) {
    state.selected = node;
    state.mode = "none";
    state.depth.clear();
    const d = data.nodes[+node];
    history.replaceState(null, "", "#" + encodeURIComponent(d.p));
    renderPanel(node, symLine);
    if (!keepCamera) {
      const p = renderer.getNodeDisplayData(node);
      if (p) {
        // Frame the file with its direct neighbours; never zoom past what that needs.
        const around = [node, ...graph.neighbors(node)];
        let x0 = p.x, y0 = p.y, x1 = p.x, y1 = p.y;
        for (const nd of around) {
          const q = renderer.getNodeDisplayData(nd);
          if (!q) continue;
          x0 = Math.min(x0, q.x); y0 = Math.min(y0, q.y); x1 = Math.max(x1, q.x); y1 = Math.max(y1, q.y);
        }
        const extent = Math.max(x1 - x0, y1 - y0) * 1.5;
        const cur = renderer.getCamera().ratio;
        const ratio = Math.min(cur, Math.max(0.12, Math.min(1, extent)));
        const { width: vw, height: vh } = renderer.getDimensions();
        const covered = window.innerWidth > 900 ? Math.min(416, vw * 0.45) : 0;
        // Wide neighbourhoods: keep the overview, only nudge the file out from under the panel.
        if (ratio > 0.7 && cur >= 0.95) {
          const at = renderer.framedGraphToViewport(renderer.getNodeDisplayData(node)!);
          if (at.x > vw - covered - 40) {
            const a = renderer.viewportToFramedGraph({ x: at.x, y: vh / 2 });
            const b = renderer.viewportToFramedGraph({ x: (vw - covered) / 2, y: vh / 2 });
            const c = renderer.getCamera().getState();
            renderer.getCamera().animate({ x: c.x + (a.x - b.x) }, { duration: 500 });
          }
          return renderer.refresh({ skipIndexation: true });
        }
        // Centre the file in the part of the canvas the panel does not cover.
        const a = renderer.viewportToFramedGraph({ x: vw / 2, y: vh / 2 });
        const b = renderer.viewportToFramedGraph({ x: (vw - covered) / 2, y: vh / 2 });
        const dx = (a.x - b.x) * (ratio / cur);
        renderer.getCamera().animate({ x: p.x + dx, y: p.y, ratio }, { duration: 600 });
      }
    }
    renderer.refresh({ skipIndexation: true });
  }

  function openHref(path: string, line?: number) {
    if (data.web && data.commit) return `${data.web}/blob/${data.commit}/${path.split("/").map(encodeURIComponent).join("/")}${line ? `#L${line}` : ""}`;
    if (data.root) return `vscode://file/${data.root}/${path}${line ? `:${line}` : ""}`;
    return "";
  }

  function fileRow(i: string, extra = "", color = "") {
    const d = data.nodes[+i];
    const dot = color ? `<span class="depth" style="background:${color}"></span>` : `<span class="depth" style="background:${commColor(d.c)}"></span>`;
    return `<div class="row" data-node="${i}">${dot}<span class="n">${esc(base(d.p))} <span class="muted">${esc(dir(d.p))}</span></span><span class="ln">${extra}</span></div>`;
  }

  function renderPanel(node: string, symLine?: number) {
    const d = data.nodes[+node];
    const comm = data.communities.find((c) => c.id === d.c);
    const ins = graph.inNeighbors(node).sort((a, b) => data.nodes[+b].r - data.nodes[+a].r);
    const outs = graph.outNeighbors(node).sort((a, b) => data.nodes[+b].r - data.nodes[+a].r);
    const pct = Math.max(1, Math.round((100 * data.nodes.filter((x) => x.r > d.r).length) / n));
    const href = openHref(d.p);
    const syms = [...d.s].sort((a, b) => a[2] - b[2]);
    if (DB) return renderTable(node, d, comm?.name ?? "", ins, outs);
    panel.innerHTML = `
      <div class="head">
        <button class="close" title="Close (Esc)">×</button>
        <div class="fname">${esc(base(d.p))}</div>
        <div class="fpath">${esc(d.p)}</div>
        <div class="chips">
          <span class="chip"><span class="dot" style="background:${commColor(d.c)}"></span>${esc(comm?.name ?? "")}</span>
          <span class="chip">${esc(d.g)}</span><span class="chip">${fmt(d.l)} lines</span>
          <span class="chip">top ${pct}% central</span>${d.t ? '<span class="chip">test</span>' : ""}
        </div>
        <div class="actions">
          <button class="btn impact" data-act="impact" title="Files that depend on this one (i)">◎ Impact</button>
          <button class="btn deps" data-act="deps" title="Files this one depends on (d)">→ Depends on</button>
          ${href ? `<a class="btn" href="${esc(href)}" target="_blank" rel="noopener">Open ↗</a>` : ""}
        </div>
      </div>
      <div class="body">
        <div id="mode"></div>
        <div id="symview"></div>
        <section><h3><span>Symbols</span><span>${syms.length}</span></h3>
          ${syms.map((s) => `<div class="row sym" data-line="${s[2]}" data-end="${s[3]}" data-name="${esc(s[0])}"><span class="k ${s[1]}">${kindLetter(s[1])}</span><span class="n">${esc(s[0])}</span><span class="ln">${s[4] ? `${s[4]}↙ ` : ""}:${s[2]}</span></div>`).join("") || '<div class="muted" style="padding:4px 8px">No symbols</div>'}
        </section>
        <section><h3><span>Used by</span><span>${ins.length}</span></h3>${ins.slice(0, 200).map((i) => fileRow(i)).join("") || '<div class="muted" style="padding:4px 8px">Nothing depends on this file</div>'}</section>
        <section><h3><span>Depends on</span><span>${outs.length}</span></h3>${outs.slice(0, 200).map((i) => fileRow(i)).join("") || '<div class="muted" style="padding:4px 8px">No internal dependencies</div>'}</section>
      </div>`;
    panel.classList.add("open");
    document.body.classList.add("panel-open");
    panel.querySelector(".close")!.addEventListener("click", clearSelection);
    panel.querySelectorAll<HTMLElement>("[data-act]").forEach((b) => b.addEventListener("click", () => setMode(b.dataset.act as "impact" | "deps")));
    if (symLine !== undefined) {
      const row = panel.querySelector<HTMLElement>(`.sym[data-line="${symLine}"]`);
      if (row) showSymbol(d, row);
    }
  }

  function renderTable(node: string, d: NodeData, group: string, ins: string[], outs: string[]) {
    const src = d.src && d.src[0] ? openHref(d.src[0], d.src[1]) : "";
    const refRow = (r: [string, number, string | null, string]) => {
      const href = openHref(r[0], r[1]);
      const inner = `<span class="k ${esc(r[3])}">${esc(r[3][0] ?? "·")}</span><span class="n">${esc(r[2] ?? base(r[0]))} <span class="muted">${esc(r[0])}</span></span><span class="ln">:${r[1]}</span>`;
      return href ? `<a class="row" href="${esc(href)}" target="_blank" rel="noopener">${inner}</a>` : `<div class="row">${inner}</div>`;
    };
    panel.innerHTML = `
      <div class="head">
        <button class="close" title="Close (Esc)">×</button>
        <div class="fname">${esc(d.p)}</div>
        <div class="fpath">${esc(d.src && d.src[0] ? `${d.src[0]}:${d.src[1]}` : data.origin ?? "")}</div>
        <div class="chips">
          <span class="chip"><span class="dot" style="background:${commColor(d.c)}"></span>${esc(group)}</span>
          <span class="chip">${esc(d.g)}</span><span class="chip">${fmt(d.l)} columns</span>
          <span class="chip">${(d.q ?? []).length} code refs</span>
        </div>
        <div class="actions">
          <button class="btn impact" data-act="impact" title="Tables that reference this one, transitively (i)">◎ Impact</button>
          <button class="btn deps" data-act="deps" title="Tables this one references (d)">→ References</button>
          ${src ? `<a class="btn" href="${esc(src)}" target="_blank" rel="noopener">Definition ↗</a>` : ""}
        </div>
      </div>
      <div class="body">
        <div id="mode"></div>
        <section><h3><span>Columns</span><span>${d.s.length}</span></h3>
          ${d.s.map((c) => `<div class="row"><span class="k ${c[1]}">${kindLetter(c[1])}</span><span class="n">${esc(c[0])}</span></div>`).join("") || '<div class="muted" style="padding:4px 8px">No columns</div>'}
        </section>
        ${(d.ix ?? []).length ? `<section><h3><span>Indexes</span><span>${d.ix!.length}</span></h3>${d.ix!.map((x) => `<div class="row"><span class="k ${x[2] ? "pk" : "column"}">${x[2] ? "U" : "i"}</span><span class="n">${esc(x[1].join(", "))} <span class="muted">${esc(x[0])}</span></span></div>`).join("")}</section>` : ""}
        <section><h3><span>Queried from</span><span>${(d.q ?? []).length}</span></h3>${(d.q ?? []).slice(0, 200).map(refRow).join("") || '<div class="muted" style="padding:4px 8px">No code references found</div>'}</section>
        <section><h3><span>Referenced by</span><span>${ins.length}</span></h3>${ins.map((i) => fileRow(i)).join("") || '<div class="muted" style="padding:4px 8px">No foreign keys point here</div>'}</section>
        <section><h3><span>References</span><span>${outs.length}</span></h3>${outs.map((i) => fileRow(i)).join("") || '<div class="muted" style="padding:4px 8px">No foreign keys</div>'}</section>
      </div>`;
    panel.classList.add("open");
    document.body.classList.add("panel-open");
    panel.querySelector(".close")!.addEventListener("click", clearSelection);
    panel.querySelectorAll<HTMLElement>("[data-act]").forEach((b) => b.addEventListener("click", () => setMode(b.dataset.act as "impact" | "deps")));
  }

  panel.addEventListener("click", (e) => {
    const row = (e.target as HTMLElement).closest(".row") as HTMLElement | null;
    if (!row) return;
    if (row.dataset.node) select(row.dataset.node);
    else if (row.classList.contains("sym") && state.selected) showSymbol(data.nodes[+state.selected], row);
  });

  async function showSymbol(d: NodeData, row: HTMLElement) {
    panel.querySelectorAll(".sym.active").forEach((r) => r.classList.remove("active"));
    row.classList.add("active");
    const line = +row.dataset.line!, end = +row.dataset.end!, name = row.dataset.name!;
    const view = panel.querySelector("#symview")!;
    const href = openHref(d.p, line);
    if (!LIVE) {
      if (href) window.open(href, "_blank", "noopener");
      return;
    }
    view.innerHTML = `<section><h3><span>${esc(name)}</span><span>:${line}</span></h3><div class="muted" style="padding:4px 8px">Loading…</div></section>`;
    try {
      const r = await fetch(`api/context?target=${encodeURIComponent(`${d.p}:${line}`)}&code_lines=${Math.min(120, end - line + 1)}`);
      const ctx = await r.json();
      const code = (ctx.code ?? "").split("\n").map((l: string, i: number) => `<span class="l"><i>${line + i}</i>${esc(l)}</span>`).join("");
      const callers = (ctx.callers ?? []).slice(0, 30);
      const callees = (ctx.callees ?? []).slice(0, 30);
      const site = (s: any, at: number) => {
        const idx = nodeByPath.get(s.symbol.file);
        return `<div class="row" ${idx !== undefined ? `data-node="${idx}"` : ""}><span class="k ${s.symbol.kind}">${kindLetter(s.symbol.kind)}</span><span class="n">${esc(s.symbol.name)} <span class="muted">${esc(base(s.symbol.file))}</span></span><span class="ln">:${at}</span></div>`;
      };
      view.innerHTML = `<section><h3><span>${esc(name)}</span>${href ? `<a href="${esc(href)}">open ↗</a>` : `<span>:${line}</span>`}</h3>
        <pre class="code">${code}</pre></section>
        ${callers.length ? `<section><h3><span>Called by</span><span>${ctx.callers.length}</span></h3>${callers.map((s: any) => site(s, s.at)).join("")}</section>` : ""}
        ${callees.length ? `<section><h3><span>Calls</span><span>${ctx.callees.length}</span></h3>${callees.map((s: any) => site(s, s.symbol.line)).join("")}</section>` : ""}`;
    } catch (err) {
      view.innerHTML = `<div class="muted" style="padding:8px">${esc(String(err))}</div>`;
    }
  }

  function bfs(start: string, dirn: "in" | "out", maxDepth: number) {
    const depth = new Map<string, number>([[start, 0]]);
    let frontier = [start];
    for (let dd = 1; dd <= maxDepth && frontier.length; dd++) {
      const next: string[] = [];
      for (const u of frontier) {
        const nb = dirn === "in" ? graph.inNeighbors(u) : graph.outNeighbors(u);
        for (const v of nb) if (!depth.has(v)) { depth.set(v, dd); next.push(v); }
      }
      frontier = next;
    }
    return depth;
  }

  function setMode(mode: "impact" | "deps") {
    if (!state.selected) return;
    state.mode = state.mode === mode ? "none" : mode;
    panel.querySelectorAll("[data-act]").forEach((b) => b.classList.toggle("on", (b as HTMLElement).dataset.act === state.mode));
    const box = panel.querySelector("#mode")!;
    if (state.mode === "none") {
      state.depth.clear();
      box.innerHTML = "";
      renderer.refresh({ skipIndexation: true });
      return;
    }
    state.depth = bfs(state.selected, state.mode === "impact" ? "in" : "out", state.mode === "impact" ? 3 : 2);
    const by: string[][] = [[], [], [], []];
    state.depth.forEach((dd, k) => { if (dd > 0) by[dd].push(k); });
    const mods = new Set([...state.depth.keys()].map((k) => data.nodes[+k].c));
    const tests = [...state.depth.keys()].filter((k) => data.nodes[+k].t).length;
    if (state.mode === "impact") {
      const total = state.depth.size - 1;
      box.innerHTML = `<div class="summary"><b>${fmt(by[1].length)}</b> ${DB ? "tables reference this directly" : "files depend on this directly"}, <b>${fmt(total - by[1].length)}</b> more indirectly — ${mods.size} ${DB ? "groups" : `modules, ${tests} test files`}.</div>
        ${[1, 2, 3].filter((k) => by[k].length).map((k) => `<section><h3><span>${k === 1 ? "Direct" : `Depth ${k}`}</span><span>${by[k].length}</span></h3>${by[k].slice(0, 80).map((i) => fileRow(i, "", DEPTH[k])).join("")}</section>`).join("")}`;
    } else {
      box.innerHTML = `<div class="summary deps"><b>${fmt(by[1].length)}</b> direct dependencies, <b>${fmt(by[2].length)}</b> at depth 2.</div>`;
    }
    zoomTo([...state.depth.keys()]);
    renderer.refresh({ skipIndexation: true });
  }

  // ---- search
  const nodeByPath = new Map<string, number>(data.nodes.map((d, i) => [d.p, i]));
  const symIndex: { i: number; name: string; lower: string; kind: string; line: number; callers: number }[] = [];
  data.nodes.forEach((d, i) => d.s.forEach((s) => symIndex.push({ i, name: s[0], lower: s[0].toLowerCase(), kind: s[1], line: s[2], callers: s[4] })));
  const input = $<HTMLInputElement>("#search input");
  const results = $("#results");
  let active = 0;
  let items: { node: number; line?: number }[] = [];
  let searchSeq = 0;

  function runSearch(q: string) {
    const ql = q.trim().toLowerCase();
    if (!ql) { results.classList.remove("open"); items = []; return; }
    const syms = symIndex
      .map((s) => {
        const last = s.lower.slice(s.lower.lastIndexOf(".") + 1);
        let sc = last === ql || s.lower === ql ? 100 : last.startsWith(ql) ? 60 : s.lower.includes(ql) ? 30 - s.lower.indexOf(ql) * 0.1 : 0;
        if (sc) sc += Math.min(20, s.callers) + data.nodes[s.i].r * 10;
        return [sc, s] as const;
      })
      .filter((x) => x[0] > 0).sort((a, b) => b[0] - a[0]).slice(0, 8);
    const files = data.nodes
      .map((d, i) => {
        const b = base(d.p).toLowerCase(), p = d.p.toLowerCase();
        let sc = b === ql || b.split(".")[0] === ql ? 90 : b.startsWith(ql) ? 55 : p.includes(ql) ? 25 : 0;
        if (sc) sc += d.r * 20;
        return [sc, i] as const;
      })
      .filter((x) => x[0] > 0).sort((a, b) => b[0] - a[0]).slice(0, 6);
    items = [];
    let html = "";
    if (syms.length) {
      html += `<div class="res-h">Symbols</div>`;
      for (const [, s] of syms) {
        items.push({ node: s.i, line: s.line });
        html += `<div class="res" data-k="${items.length - 1}"><span class="k">${kindLetter(s.kind)}</span><span class="n">${esc(s.name)}</span><span class="p">${esc(data.nodes[s.i].p)}:${s.line}</span></div>`;
      }
    }
    if (files.length) {
      html += `<div class="res-h">Files</div>`;
      for (const [, i] of files) {
        items.push({ node: i });
        html += `<div class="res" data-k="${items.length - 1}"><span class="k">▢</span><span class="n">${esc(base(data.nodes[i].p))}</span><span class="p">${esc(data.nodes[i].p)}</span></div>`;
      }
    }
    if (!html && !LIVE) html = `<div class="res-h">No matches</div>`;
    results.innerHTML = html + (LIVE ? `<div id="code-res"></div>` : "");
    results.classList.add("open");
    active = 0;
    paintActive();
    if (LIVE) codeSearch(q);
  }

  let codeTimer = 0;
  function codeSearch(q: string) {
    clearTimeout(codeTimer);
    const seq = ++searchSeq;
    codeTimer = window.setTimeout(async () => {
      try {
        const r = await fetch(`api/search?q=${encodeURIComponent(q)}&limit=8`);
        const res = await r.json();
        if (seq !== searchSeq) return;
        const box = $("#code-res");
        if (!box) return;
        const hits = (res.hits ?? []).filter((h: any) => nodeByPath.has(h.file));
        if (!hits.length) return;
        let html = `<div class="res-h">In code</div>`;
        for (const h of hits) {
          items.push({ node: nodeByPath.get(h.file)!, line: h.symbol?.line ?? h.start });
          const line = (h.snippet ?? []).find((s: any) => (h.matched ?? []).some((m: string) => s[1].toLowerCase().includes(m))) ?? h.snippet?.[0];
          html += `<div class="res" data-k="${items.length - 1}"><span class="k">≡</span><span class="n">${esc(h.symbol?.name ?? base(h.file))}</span><span class="p">${esc(h.file)}:${h.start}</span></div>`;
          if (line) html += `<div class="res snip-row" data-k="${items.length - 1}" style="padding-top:0"><span class="k"></span><span class="snip">${esc(line[1].trim())}</span></div>`;
        }
        box.innerHTML = html;
      } catch {}
    }, 160);
  }

  function paintActive() {
    results.querySelectorAll(".res").forEach((el) => el.classList.toggle("active", +(el as HTMLElement).dataset.k! === active && !el.classList.contains("snip-row")));
    results.querySelector(".res.active")?.scrollIntoView({ block: "nearest" });
  }
  function choose(k: number) {
    const it = items[k];
    if (!it) return;
    results.classList.remove("open");
    input.blur();
    select(String(it.node), false, it.line);
  }
  input.addEventListener("input", () => runSearch(input.value));
  input.addEventListener("focus", () => input.value && runSearch(input.value));
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") { active = Math.min(items.length - 1, active + 1); paintActive(); e.preventDefault(); }
    else if (e.key === "ArrowUp") { active = Math.max(0, active - 1); paintActive(); e.preventDefault(); }
    else if (e.key === "Enter") choose(active);
    else if (e.key === "Escape") { results.classList.remove("open"); input.blur(); }
  });
  results.addEventListener("mousedown", (e) => {
    const el = (e.target as HTMLElement).closest(".res") as HTMLElement | null;
    if (el) { e.preventDefault(); choose(+el.dataset.k!); }
  });
  document.addEventListener("click", (e) => {
    if (!(e.target as HTMLElement).closest("#search")) results.classList.remove("open");
  });

  // ---- controls
  function setBtn(id: string, on: boolean) {
    document.querySelector(`#controls [data-c="${id}"]`)?.classList.toggle("on", on);
  }
  setBtn("edges", state.edges);
  setBtn("tests", state.tests);
  setBtn("labels", state.labels);
  $("#controls").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("button") as HTMLElement | null;
    if (!b) return;
    const c = b.dataset.c;
    if (c === "layout") layout.isRunning() ? stopLayout() : runLayout(8000);
    else if (c === "fit") renderer.getCamera().animatedReset({ duration: 500 });
    else if (c === "edges") { state.edges = !state.edges; setBtn("edges", state.edges); }
    else if (c === "tests") { state.tests = !state.tests; setBtn("tests", state.tests); }
    else if (c === "labels") { state.labels = !state.labels; setBtn("labels", state.labels); }
    renderer.refresh({ skipIndexation: true });
  });
  document.addEventListener("keydown", (e) => {
    const typing = document.activeElement === input;
    if ((e.key === "k" && (e.metaKey || e.ctrlKey)) || (e.key === "/" && !typing)) { input.focus(); input.select(); e.preventDefault(); return; }
    if (typing) return;
    if (e.key === "Escape") clearSelection();
    else if (e.key === "i") setMode("impact");
    else if (e.key === "d") setMode("deps");
    else if (e.key === "f") renderer.getCamera().animatedReset({ duration: 500 });
  });

  // Deep link: #path/to/file
  const hash = decodeURIComponent(location.hash.slice(1));
  if (hash && nodeByPath.has(hash)) setTimeout(() => select(String(nodeByPath.get(hash))), 400);

  $("#loading").classList.add("done");
  (window as any).repomap = { graph, renderer, select: (p: string) => nodeByPath.has(p) && select(String(nodeByPath.get(p))), impact: () => setMode("impact"), stopLayout, focus: (c: number) => { state.focusComm = c; renderer.refresh(); } };
}

function kindLetter(k: string) {
  return ({ function: "ƒ", method: "m", class: "C", struct: "S", interface: "I", trait: "T", enum: "E", type: "t", module: "M", macro: "!", pk: "K", fk: "→", column: "c" } as Record<string, string>)[k] ?? "·";
}

/** Opaque blend of `hex` over the background (WebGL alpha blending varies by GPU). */
function mix(hex: string, a: number) {
  const v = parseInt(hex.slice(1), 16);
  const bg = [6, 8, 12];
  const c = [(v >> 16) & 255, (v >> 8) & 255, v & 255].map((x, i) => Math.round(bg[i] + (x - bg[i]) * a));
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

function hexA(hex: string, a: number) {
  const v = parseInt(hex.slice(1), 16);
  return `rgba(${(v >> 16) & 255},${(v >> 8) & 255},${v & 255},${a})`;
}

function drawLabel(ctx: CanvasRenderingContext2D, d: any, s: any) {
  if (!d.label) return;
  const size = s.labelSize;
  ctx.font = `${s.labelWeight} ${size}px ${s.labelFont}`;
  ctx.shadowColor = "rgba(0,0,0,0.95)";
  ctx.shadowBlur = 6;
  ctx.fillStyle = d.highlighted ? "#ffffff" : "#c9d1e3";
  ctx.fillText(d.label, d.x + d.size + 4, d.y + size / 3);
  ctx.shadowBlur = 0;
}

function drawHover(ctx: CanvasRenderingContext2D, d: any, s: any) {
  ctx.beginPath();
  ctx.arc(d.x, d.y, d.size + 3, 0, Math.PI * 2);
  ctx.strokeStyle = "rgba(255,255,255,0.9)";
  ctx.lineWidth = 1.5;
  ctx.stroke();
  drawLabel(ctx, { ...d, highlighted: true }, s);
}

load()
  .then(mount)
  .catch((err) => {
    $("#loading").innerHTML = `<div style="max-width:520px;text-align:center">Could not load the map.<br><span class="muted">${esc(String(err))}</span></div>`;
  });
