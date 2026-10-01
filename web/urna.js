"use strict";
// Plugin do macroquad: WebSocket, WebAudio, YouTube no telão e prompt. Funções importadas em src/web.rs.
(function () {
    const enc = new TextEncoder(), dec = new TextDecoder();
    const str = (p, n) => dec.decode(new Uint8Array(wasm_memory.buffer, p, n));
    // Escreve bytes em (ptr, cap): retorna tamanho, ou -tamanho se não coube.
    function put(bytes, ptr, cap) {
        if (bytes.length > cap) return -bytes.length;
        new Uint8Array(wasm_memory.buffer, ptr, bytes.length).set(bytes);
        return bytes.length;
    }

    // ---------------------------------------------------------------- WebSocket
    let ws = null, wsState = 2;
    const inbox = [];
    function connect(url) {
        if (!url) url = (location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/ws";
        try { ws = new WebSocket(url); } catch (e) { wsState = 2; return; }
        wsState = 0;
        ws.onopen = () => { wsState = 1; };
        ws.onclose = ws.onerror = () => { wsState = 2; };
        ws.onmessage = (e) => inbox.push(enc.encode(e.data));
    }

    // ---------------------------------------------------------------- Áudio
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const clips = [], voices = new Map();
    let nextVoice = 1;
    const master = ctx.createGain();
    master.connect(ctx.destination);

    // ---------------------------------------------------------------- Clipe (tecla B / botão CLIPE)
    // Pedaços de webm/mp4 depois do primeiro não tocam sozinhos, então "últimos 10s" = dois gravadores
    // revezando: o novo nasce a cada 10s e o mais velho (10-20s de vídeo) é o que vira arquivo.
    const CLIP_MS = 10000;
    const clipMsgs = [];
    const clipMime = typeof MediaRecorder !== "undefined" && HTMLCanvasElement.prototype.captureStream
        ? ["video/mp4;codecs=avc1,mp4a.40.2", "video/mp4", "video/webm;codecs=vp8,opus", "video/webm"].find((t) => MediaRecorder.isTypeSupported(t)) : null;
    const weakDevice = navigator.maxTouchPoints > 0 && ((navigator.deviceMemory || 8) <= 4 || (navigator.hardwareConcurrency || 8) <= 4);
    const CAPTION = "olha isso na URNA-MINE-VERINE: https://urna-mine-verine.wilianzilv.workers.dev";
    let clipTier = -1, older = null, newer = null, clipBusy = false, videoTrack = null, audioTap = null, captionOk = false;
    const rolling = () => !!clipMime && clipTier > 0 && !weakDevice && !clipBusy && document.visibilityState === "visible";
    // Marca d'água: grava de um canvas 2D que copia o jogo a cada quadro, só enquanto há trilha viva
    // (qualidade BAIXA grava o canvas direto, sem a cópia).
    function makeVideoTrack() {
        const gl = document.getElementById("glcanvas");
        if (clipTier <= 0) return gl.captureStream(30).getVideoTracks()[0];
        const c = document.createElement("canvas"), g = c.getContext("2d", { alpha: false });
        const track = c.captureStream(60).getVideoTracks()[0];
        const draw = () => {
            if (track.readyState !== "live") return;
            const s = Math.min(1, 1920 / gl.width), w = gl.width * s & ~1, h = gl.height * s & ~1;
            if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
            g.drawImage(gl, 0, 0, w, h);
            const px = Math.max(12, h * 0.028 | 0);
            g.font = `bold ${px}px monospace`;
            g.textAlign = "right";
            g.lineWidth = Math.max(2, px / 5);
            g.strokeStyle = "rgba(0,0,0,0.7)";
            g.fillStyle = "rgba(255,255,255,0.85)";
            g.strokeText("urna-mine-verine.workers.dev", w - px * 0.6, h - px * 0.6);
            g.fillText("urna-mine-verine.workers.dev", w - px * 0.6, h - px * 0.6);
            requestAnimationFrame(draw);
        };
        // Depois do rAF do macroquad no mesmo quadro: o buffer WebGL ainda tem a imagem nova.
        setTimeout(() => requestAnimationFrame(draw));
        return track;
    }
    function clipTracks() {
        if (!videoTrack || videoTrack.readyState === "ended") videoTrack = makeVideoTrack();
        const t = [videoTrack];
        if (ctx.createMediaStreamDestination) {
            if (!audioTap) { audioTap = ctx.createMediaStreamDestination(); master.connect(audioTap); }
            t.push(audioTap.stream.getAudioTracks()[0]);
        }
        return t;
    }
    function startSeg() {
        const seg = { chunks: [], t0: performance.now(), mute: ctx.state !== "running" };
        seg.rec = new MediaRecorder(new MediaStream(clipTracks()), {
            mimeType: clipMime, videoBitsPerSecond: weakDevice ? 2500000 : 8000000, audioBitsPerSecond: 160000,
            videoKeyFrameIntervalDuration: 1000,
        });
        seg.rec.ondataavailable = (e) => { if (e.data.size) seg.chunks.push(e.data); };
        seg.rec.start(1000);
        return seg;
    }
    function dropSeg(seg) { if (seg && seg.rec.state !== "inactive") { seg.rec.ondataavailable = null; seg.rec.stop(); } }
    const stopSeg = (seg) => new Promise((r) => { seg.rec.onstop = () => { seg.ms = performance.now() - seg.t0; r(seg); }; seg.rec.stop(); });
    function roll() {
        if (!rolling()) {
            if (clipBusy) return;
            dropSeg(older); dropSeg(newer);
            older = newer = null;
            if (videoTrack) { videoTrack.stop(); videoTrack = null; }
            return;
        }
        if (!newer) newer = startSeg();
        else if (performance.now() - newer.t0 >= CLIP_MS) { dropSeg(older); older = newer; newer = startSeg(); }
    }
    setInterval(roll, 500);
    document.addEventListener("visibilitychange", roll);
    // Gravador que nasceu com o áudio travado (antes do 1º clique) põe o som atrasado no começo do arquivo: recomeça.
    ctx.addEventListener("statechange", () => {
        if (ctx.state !== "running" || clipBusy) return;
        if (older && older.mute) { dropSeg(older); older = null; }
        if (newer && newer.mute) { dropSeg(newer); newer = null; }
        roll();
    });
    function recDot(on) { const d = document.getElementById("rec"); if (d) d.style.display = on ? "block" : "none"; }
    // MediaRecorder solta MP4 fragmentado (moov vazio, sem duração: player não mostra tempo nem deixa pular).
    // Remonta como MP4 normal com tabelas completas, cortando pros últimos `keepSec` a partir de um quadro-chave.
    function fixMp4(buf, keepSec) {
        const u8 = new Uint8Array(buf), dv = new DataView(buf);
        const boxes = (s, e) => {
            const out = [];
            while (s + 8 <= e) {
                let n = dv.getUint32(s), h = 8;
                if (n === 1) { n = Number(dv.getBigUint64(s + 8)); h = 16; } else if (n === 0) n = e - s;
                if (n < h || s + n > e) break;
                out.push({ t: String.fromCharCode(...u8.subarray(s + 4, s + 8)), s, e: s + n, d: s + h });
                s += n;
            }
            return out;
        };
        const kids = (b) => (b ? boxes(b.d, b.e) : []);
        const get = (b, ...path) => path.reduce((x, t) => kids(x).find((k) => k.t === t), b);
        const ver = (b) => u8[b.d], raw = (b) => u8.subarray(b.s, b.e);
        const top = boxes(0, u8.length), moov = top.find((b) => b.t === "moov");
        if (!moov || !top.some((b) => b.t === "moof")) return null;
        const mvhd = get(moov, "mvhd"), movieScale = dv.getUint32(mvhd.d + (ver(mvhd) ? 20 : 12));
        const tracks = new Map();
        for (const trak of kids(moov).filter((b) => b.t === "trak")) {
            const tkhd = get(trak, "tkhd"), mdhd = get(trak, "mdia", "mdhd"), hdlr = get(trak, "mdia", "hdlr");
            tracks.set(dv.getUint32(tkhd.d + (ver(tkhd) ? 20 : 12)), {
                trak, scale: dv.getUint32(mdhd.d + (ver(mdhd) ? 20 : 12)), s: [], def: [0, 0, 0], next: 0,
                video: String.fromCharCode(...u8.subarray(hdlr.d + 8, hdlr.d + 12)) === "vide",
            });
        }
        for (const trex of kids(get(moov, "mvex")).filter((b) => b.t === "trex")) {
            const tr = tracks.get(dv.getUint32(trex.d + 4));
            if (tr) tr.def = [dv.getUint32(trex.d + 12), dv.getUint32(trex.d + 16), dv.getUint32(trex.d + 20)];
        }
        for (const moof of top.filter((b) => b.t === "moof")) for (const traf of kids(moof).filter((b) => b.t === "traf")) {
            const tfhd = get(traf, "tfhd"), tfdt = get(traf, "tfdt"), fl = dv.getUint32(tfhd.d) & 0xffffff;
            const tr = tracks.get(dv.getUint32(tfhd.d + 4));
            if (!tr) continue;
            let q = tfhd.d + 8, base = moof.s, [dDur, dSize, dFlags] = tr.def;
            const rd = () => { q += 4; return dv.getUint32(q - 4); };
            if (fl & 1) { base = Number(dv.getBigUint64(q)); q += 8; }
            if (fl & 2) q += 4;
            if (fl & 8) dDur = rd();
            if (fl & 0x10) dSize = rd();
            if (fl & 0x20) dFlags = rd();
            let t = tfdt ? (ver(tfdt) ? Number(dv.getBigUint64(tfdt.d + 4)) : dv.getUint32(tfdt.d + 4)) : tr.next, off = base;
            for (const trun of kids(traf).filter((b) => b.t === "trun")) {
                q = trun.d;
                const rf = rd() & 0xffffff, n = rd();
                if (rf & 1) off = base + (rd() | 0);
                const first = rf & 4 ? rd() : null;
                for (let i = 0; i < n; i++) {
                    const dur = rf & 0x100 ? rd() : dDur, size = rf & 0x200 ? rd() : dSize;
                    const flags = rf & 0x400 ? rd() : i === 0 && first !== null ? first : dFlags, cto = rf & 0x800 ? rd() | 0 : 0;
                    tr.s.push({ off, size, dur, t, cto, sync: !tr.video || !(flags & 0x10000) });
                    off += size;
                    t += dur;
                }
            }
            tr.next = t;
        }
        const all = [...tracks.values()].filter((tr) => tr.s.length), vid = all.find((tr) => tr.video);
        const endOf = (tr) => { const l = tr.s[tr.s.length - 1]; return (l.t + l.dur) / tr.scale; };
        const end = vid ? endOf(vid) : Math.max(...all.map(endOf));
        let cut = vid ? vid.s[0].t / vid.scale : 0;
        if (vid) for (const x of vid.s) if (x.sync && x.t / vid.scale <= end - keepSec) cut = x.t / vid.scale;
        const chunks = [];
        for (const tr of tracks.values()) {
            tr.s = tr.s.filter((x) => (tr.video ? x.t / tr.scale >= cut : (x.t + x.dur) / tr.scale > cut && x.t / tr.scale < end));
            // stts é contínuo: duração = distância até a próxima amostra, senão os buracos entre fragmentos dessincronizam.
            tr.s.forEach((x, i) => { const n = tr.s[i + 1]; if (n && n.t > x.t) x.dur = n.t - x.t; });
            tr.dur = tr.s.reduce((a, x) => a + x.dur, 0);
            tr.chunks = [];
            tr.s.forEach((x, i) => {
                const prev = tr.s[i - 1], c = tr.chunks[tr.chunks.length - 1];
                if (prev && prev.off + prev.size === x.off) { c.len += x.size; c.n++; } else tr.chunks.push({ src: x.off, len: x.size, n: 1 });
            });
            chunks.push(...tr.chunks);
        }
        chunks.sort((a, b) => a.src - b.src);
        let pos = 0;
        for (const c of chunks) { c.pos = pos; pos += c.len; }
        const box = (t, ...parts) => {
            const o = new Uint8Array(8 + parts.reduce((a, p) => a + p.length, 0));
            new DataView(o.buffer).setUint32(0, o.length);
            for (let i = 0; i < 4; i++) o[4 + i] = t.charCodeAt(i);
            let p = 8;
            for (const x of parts) { o.set(x, p); p += x.length; }
            return o;
        };
        const u32s = (v) => { const o = new Uint8Array(v.length * 4), w = new DataView(o.buffer); v.forEach((x, i) => w.setUint32(i * 4, x)); return o; };
        const withDur = (b, at, v) => {
            const o = raw(b).slice(), w = new DataView(o.buffer);
            if (ver(b)) w.setBigUint64(at - b.s, BigInt(Math.round(v))); else w.setUint32(at - b.s, Math.round(v));
            return o;
        };
        const rle = (vals) => { const r = []; for (const v of vals) { if (r.length && r[r.length - 1] === v) r[r.length - 2]++; else r.push(1, v); } return r; };
        const stbl = (tr, stsd, base) => {
            const stts = rle(tr.s.map((x) => x.dur)), ctts = rle(tr.s.map((x) => x.cto >>> 0));
            const stss = tr.s.flatMap((x, i) => (x.sync ? [i + 1] : [])), stsc = [];
            tr.chunks.forEach((c, i) => { if (!i || tr.chunks[i - 1].n !== c.n) stsc.push(i + 1, c.n, 1); });
            return box("stbl", raw(stsd),
                box("stts", u32s([0, stts.length / 2].concat(stts))),
                ...(tr.s.some((x) => x.cto) ? [box("ctts", u32s([1 << 24, ctts.length / 2].concat(ctts)))] : []),
                ...(tr.video && stss.length < tr.s.length ? [box("stss", u32s([0, stss.length].concat(stss)))] : []),
                box("stsc", u32s([0, stsc.length / 3].concat(stsc))),
                box("stsz", u32s([0, 0, tr.s.length].concat(tr.s.map((x) => x.size)))),
                box("co64", u32s([0, tr.chunks.length].concat(tr.chunks.flatMap((c) => { const v = base + c.pos; return [Math.floor(v / 4294967296), v >>> 0]; })))));
        };
        const byTrak = new Map([...tracks.values()].map((tr) => [tr.trak.s, tr]));
        const movieDur = Math.max(...[...tracks.values()].map((tr) => tr.dur / tr.scale * movieScale));
        const build = (base) => box("moov", ...kids(moov).filter((b) => b.t !== "mvex").map((b) => {
            if (b.t === "mvhd") return withDur(b, b.d + (ver(b) ? 24 : 16), movieDur);
            if (b.t !== "trak") return raw(b);
            const tr = byTrak.get(b.s);
            return box("trak", ...kids(b).filter((k) => k.t !== "edts").map((k) =>
                k.t === "tkhd" ? withDur(k, k.d + (ver(k) ? 28 : 20), tr.dur / tr.scale * movieScale)
                    : k.t !== "mdia" ? raw(k)
                        : box("mdia", ...kids(k).map((m) =>
                            m.t === "mdhd" ? withDur(m, m.d + (ver(m) ? 24 : 16), tr.dur)
                                : m.t !== "minf" ? raw(m)
                                    : box("minf", ...kids(m).map((n) => (n.t === "stbl" ? stbl(tr, get(n, "stsd"), base) : raw(n))))))));
        }));
        const head = u8.subarray(0, moov.s), mdatAt = head.length + build(0).length;
        return new Blob([head, build(mdatAt + 8), u32s([8 + pos, 0x6d646174]), ...chunks.map((c) => u8.subarray(c.src, c.src + c.len))], { type: "video/mp4" });
    }
    // WebM do MediaRecorder sai sem Duration: enfia o elemento no Info (Segment de tamanho aberto, sem SeekHead).
    function fixWebm(buf, ms) {
        const u8 = new Uint8Array(buf);
        const idLen = (p) => Math.clz32(u8[p]) - 23;
        const vint = (p) => {
            const len = Math.clz32(u8[p]) - 23;
            let v = u8[p] & (0xff >> len);
            for (let i = 1; i < len; i++) v = v * 256 + u8[p + i];
            return { len, v, open: v === 2 ** (7 * len) - 1 };
        };
        const elem = (p) => { const il = idLen(p), sz = vint(p + il); return { id: u8.subarray(p, p + il).join(","), d: p + il + sz.len, size: sz.v, open: sz.open }; };
        const ebml = elem(0), seg = elem(ebml.d + ebml.size);
        if (seg.id !== "24,83,128,103" || !seg.open) return null;
        for (let p = seg.d; p < u8.length;) {
            const e = elem(p);
            if (e.id === "17,77,155,116" || e.open) return null;
            if (e.id !== "21,73,169,102") { p = e.d + e.size; continue; }
            let scale = 1e6;
            for (let q = e.d; q < e.d + e.size;) {
                const c = elem(q);
                if (c.id === "68,137") return null;
                if (c.id === "42,215,177") { scale = 0; for (let i = 0; i < c.size; i++) scale = scale * 256 + u8[c.d + i]; }
                q = c.d + c.size;
            }
            const info = new Uint8Array(4 + 8 + e.size + 11), w = new DataView(info.buffer);
            info.set([0x15, 0x49, 0xa9, 0x66, 0x01, 0, 0, 0]);
            w.setUint32(8, e.size + 11);
            info.set(u8.subarray(e.d, e.d + e.size), 12);
            info.set([0x44, 0x89, 0x88], 12 + e.size);
            w.setFloat64(15 + e.size, ms * 1e6 / scale);
            return new Blob([u8.subarray(0, p), info, u8.subarray(e.d + e.size)], { type: "video/webm" });
        }
        return null;
    }
    async function saveSeg(seg) {
        let blob = new Blob(seg.chunks, { type: clipMime.split(";")[0] });
        if (blob.size < 1000) { clipMsgs.push("CLIPE: deu ruim, video saiu vazio"); return; }
        try {
            const buf = await blob.arrayBuffer();
            blob = (clipMime.startsWith("video/mp4") ? fixMp4(buf, CLIP_MS / 1000) : fixWebm(buf, seg.ms)) || blob;
        } catch (e) { console.warn("clipe: remux falhou, salvando cru", e); }
        const d = new Date(), p = (n) => String(n).padStart(2, "0");
        const name = `urna-clip-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}.${clipMime.startsWith("video/mp4") ? "mp4" : "webm"}`;
        const a = Object.assign(document.createElement("a"), { href: URL.createObjectURL(blob), download: name });
        document.body.appendChild(a);
        a.click();
        a.remove();
        setTimeout(() => URL.revokeObjectURL(a.href), 60000);
        clipMsgs.push(`CLIPE SALVO: ${name} - ${captionOk ? "legenda copiada, " : ""}posta onde quiser`);
    }
    function clipSave() {
        if (!clipMime) return clipMsgs.push("CLIPE: esse navegador nao grava video");
        if (clipBusy) return clipMsgs.push("CLIPE: calma, ainda gravando o anterior");
        // Clipboard exige gesto recente: este é o quadro logo após a tecla/toque.
        captionOk = false;
        try { navigator.clipboard.writeText(CAPTION).then(() => { captionOk = true; }, () => { }); } catch (e) { }
        const seg = older || newer;
        if (seg) {
            older = seg === older ? newer : null;
            newer = startSeg();
            stopSeg(seg).then(saveSeg);
            return;
        }
        clipBusy = true;
        recDot(true);
        clipMsgs.push("REC: gravando os proximos 10s...");
        const now = startSeg();
        setTimeout(() => stopSeg(now).then((s) => { clipBusy = false; recDot(false); saveSeg(s); }), CLIP_MS);
    }

    // ---------------------------------------------------------------- YouTube
    let player = null, ytReady = false, pendingId = null, ytState = -1, ytTitle = "", titleSent = "", lastVol = -1;
    function videoId(src) {
        try {
            const u = new URL(src);
            if (u.hostname.includes("youtu.be")) return u.pathname.slice(1);
            const v = u.searchParams.get("v");
            if (v) return v;
            const m = u.pathname.match(/\/(embed|shorts|live)\/([^/?]+)/);
            if (m) return m[2];
        } catch (e) { }
        return /^[\w-]{11}$/.test(src) ? src : null;
    }
    function ytLoad(id) {
        if (!id) return;
        pendingId = id;
        loadThumbs(id);
        if (ytReady) player.loadVideoById(id);
    }
    // Todo mundo na mesma linha do tempo: servidor manda há quantos segundos o vídeo começou; segue
    // pelo relógio local (sem depender do relógio do PC) e corrige quando desvia mais de 2 s.
    let syncBase = null;
    function ytSync(elapsed) {
        syncBase = performance.now() / 1000 - elapsed;
        ytResync();
    }
    function ytResync() {
        if (!ytReady || ytState !== 1 || syncBase === null) return;
        const dur = player.getDuration() || 0, d = player.getVideoData();
        if (dur <= 0 || (d && d.isLive)) return;
        const t = (performance.now() / 1000 - syncBase) % dur;
        if (Math.abs(player.getCurrentTime() - t) > 2) player.seekTo(t, true);
    }
    setInterval(ytResync, 5000);
    // Cores do vídeo pras luzes do clube: o iframe é de outra origem (pixel ilegível), então mistura as
    // miniaturas do YouTube (capa + quadros a 25/50/75%) pela posição do player. Passam pelo proxy /api/ytthumb.
    const thumbCtx = Object.assign(document.createElement("canvas"), { width: 8, height: 8 }).getContext("2d", { willReadFrequently: true });
    let thumbs = [], thumbId = null;
    function loadThumbs(id) {
        if (id === thumbId) return;
        thumbId = id;
        thumbs = [];
        ["hqdefault", "1", "2", "3"].forEach((n, k) => {
            const img = new Image();
            img.crossOrigin = "anonymous";
            img.onload = () => {
                if (thumbId !== id) return;
                try {
                    const h = img.height * 0.75; // corta as tarjas pretas (16:9 dentro do 4:3)
                    thumbCtx.drawImage(img, 0, (img.height - h) / 2, img.width, h, 0, 0, 8, 8);
                    const d = thumbCtx.getImageData(0, 0, 8, 8).data, rgb = new Uint8Array(192);
                    for (let i = 0; i < 64; i++) rgb.set(d.subarray(i * 4, i * 4 + 3), i * 3);
                    thumbs[k] = rgb;
                } catch (e) { }
            };
            img.onerror = () => { if (thumbId === id && !img.src.includes("ytimg")) img.src = `https://i.ytimg.com/vi/${id}/${n}.jpg`; };
            img.src = `/api/ytthumb/${id}/${n}`;
        });
    }
    function ytColors() {
        const first = thumbs.find((t) => t);
        if (ytState !== 1 || !first) return null;
        const dur = player.getDuration() || 0;
        const pos = dur > 0 ? ((player.getCurrentTime() / dur) % 1) * 4 : 0;
        const a = Math.floor(pos) % 4, t = pos - Math.floor(pos);
        const A = thumbs[a] || first, B = thumbs[(a + 1) % 4] || first, out = new Uint8Array(192);
        for (let i = 0; i < 192; i++) out[i] = A[i] + (B[i] - A[i]) * t;
        return out;
    }
    window.onYouTubeIframeAPIReady = function () {
        player = new YT.Player("yt", {
            width: 640, height: 360,
            playerVars: { autoplay: 1, controls: 0, disablekb: 1, playsinline: 1, rel: 0 },
            events: {
                onReady: () => { ytReady = true; if (pendingId) player.loadVideoById(pendingId); },
                onStateChange: (e) => {
                    ytState = e.data;
                    if (e.data === 0) player.seekTo(0);
                    if (e.data === 1) { const d = player.getVideoData(); ytTitle = (d && d.title) || ""; ytResync(); }
                },
            },
        });
    };
    // Navegador só libera som depois de um clique/tecla.
    function gesture() {
        if (ctx.state !== "running") ctx.resume();
        if (ytReady && ytState !== 1) { player.unMute(); player.playVideo(); }
    }
    window.addEventListener("pointerdown", gesture);
    window.addEventListener("keydown", gesture);
    // Celular: tela cheia deitada no primeiro toque, sem zoom/scroll/menu
    let fullscreenTried = false;
    window.addEventListener("touchstart", () => {
        if (fullscreenTried) return;
        fullscreenTried = true;
        const el = document.documentElement;
        const req = el.requestFullscreen || el.webkitRequestFullscreen;
        if (req) Promise.resolve(req.call(el)).then(() => screen.orientation && screen.orientation.lock && screen.orientation.lock("landscape").catch(() => { })).catch(() => { });
    }, { passive: true });
    window.addEventListener("contextmenu", (e) => e.preventDefault());

    // Homografia: retângulo 640x360 -> 4 pontos na tela (CSS matrix3d).
    function adj(m) {
        return [m[4] * m[8] - m[5] * m[7], m[2] * m[7] - m[1] * m[8], m[1] * m[5] - m[2] * m[4],
        m[5] * m[6] - m[3] * m[8], m[0] * m[8] - m[2] * m[6], m[2] * m[3] - m[0] * m[5],
        m[3] * m[7] - m[4] * m[6], m[1] * m[6] - m[0] * m[7], m[0] * m[4] - m[1] * m[3]];
    }
    function mul(a, b) {
        const c = new Array(9);
        for (let i = 0; i < 3; i++) for (let j = 0; j < 3; j++) {
            let s = 0;
            for (let k = 0; k < 3; k++) s += a[3 * i + k] * b[3 * k + j];
            c[3 * i + j] = s;
        }
        return c;
    }
    function mulv(m, v) {
        return [m[0] * v[0] + m[1] * v[1] + m[2] * v[2], m[3] * v[0] + m[4] * v[1] + m[5] * v[2], m[6] * v[0] + m[7] * v[1] + m[8] * v[2]];
    }
    function basis(p) {
        const m = [p[0], p[2], p[4], p[1], p[3], p[5], 1, 1, 1];
        const v = mulv(adj(m), [p[6], p[7], 1]);
        return mul(m, [v[0], 0, 0, 0, v[1], 0, 0, 0, v[2]]);
    }
    const SRC = basis([0, 0, 640, 0, 640, 360, 0, 360]);
    function matrix3d(dst) {
        const t = mul(basis(dst), adj(SRC)).map((x, _, a) => x / a[8]);
        return "matrix3d(" + [t[0], t[3], 0, t[6], t[1], t[4], 0, t[7], 0, 0, 1, 0, t[2], t[5], 0, t[8]].join(",") + ")";
    }

    // Celular fora da qualidade ALTA: WebGL sem MSAA (o contexto é criado uma vez só, no load).
    const touch = navigator.maxTouchPoints > 0 && matchMedia("(pointer: coarse)").matches;
    let savedQ = null;
    try { savedQ = localStorage.getItem("urna_q"); } catch (e) { }
    if (touch && (new URLSearchParams(location.search).get("q") || savedQ) !== "high") {
        const getContext = HTMLCanvasElement.prototype.getContext;
        HTMLCanvasElement.prototype.getContext = function (type, attrs) {
            return getContext.call(this, type, /webgl/.test(type) ? Object.assign({ antialias: false }, attrs) : attrs);
        };
    }

    let pendingPrompt = null;

    miniquad_add_plugin({
        register_plugin: function (io) {
            Object.assign(io.env, {
                urna_ws_connect: (p, n) => connect(str(p, n)),
                urna_ws_state: () => wsState,
                urna_ws_send: (p, n) => { if (ws && ws.readyState === 1) ws.send(str(p, n)); },
                urna_ws_recv: (p, cap) => {
                    if (!inbox.length) return 0;
                    const r = put(inbox[0], p, cap);
                    if (r > 0) inbox.shift();
                    return r;
                },
                urna_audio_rate: () => ctx.sampleRate | 0,
                urna_audio_clip: (p, n, rate) => {
                    const buf = ctx.createBuffer(1, n, rate);
                    buf.copyToChannel(new Float32Array(new Float32Array(wasm_memory.buffer, p, n)), 0);
                    clips.push(buf);
                    return clips.length - 1;
                },
                urna_audio_play: (clip, vol, loop) => {
                    const src = ctx.createBufferSource();
                    src.buffer = clips[clip];
                    src.loop = !!loop;
                    const g = ctx.createGain();
                    g.gain.value = vol;
                    src.connect(g).connect(master);
                    src.start();
                    const id = nextVoice++;
                    voices.set(id, g);
                    src.onended = () => voices.delete(id);
                    return id;
                },
                urna_audio_volume: (id, vol) => { const g = voices.get(id); if (g) g.gain.value = vol; },
                urna_yt_load: (p, n) => ytLoad(videoId(str(p, n).trim())),
                urna_yt_state: () => ytState,
                urna_yt_sync: (elapsed) => ytSync(elapsed),
                urna_yt_title: (p, cap) => {
                    if (!ytTitle || ytTitle === titleSent) return 0;
                    const r = put(enc.encode(ytTitle), p, cap);
                    if (r > 0) titleSent = ytTitle;
                    return r;
                },
                urna_yt_colors: (p, cap) => { const c = ytColors(); return c ? put(c, p, cap) : 0; },
                urna_yt_place: (x0, y0, x1, y1, x2, y2, x3, y3, visible, vol) => {
                    const wrap = document.getElementById("ytwrap");
                    if (visible) {
                        wrap.style.transform = matrix3d([x0, y0, x1, y1, x2, y2, x3, y3]);
                        wrap.style.visibility = "visible";
                    } else {
                        wrap.style.visibility = "hidden";
                    }
                    const v = Math.round(vol * 100);
                    if (ytReady && Math.abs(v - lastVol) > 2) { player.setVolume(v); lastVol = v; }
                },
                urna_open_url: (p, n) => {
                    const url = str(p, n);
                    if (document.exitPointerLock) document.exitPointerLock();
                    // Sem a flag "noopener" (com ela o open sempre devolve null); corta o opener na mão.
                    const w = window.open(url, "_blank");
                    if (w) { w.opener = null; return 1; }
                    if (navigator.clipboard) navigator.clipboard.writeText(url).catch(() => { });
                    return 0;
                },
                urna_clip_save: () => { clipSave(); },
                urna_clip_poll: (tier, p, cap) => {
                    clipTier = tier;
                    if (!clipMsgs.length) return 0;
                    const r = put(enc.encode(clipMsgs[0]), p, cap);
                    if (r > 0) clipMsgs.shift();
                    return r;
                },
                urna_is_touch: () => (navigator.maxTouchPoints > 0 && matchMedia("(pointer: coarse)").matches) ? 1 : 0,
                urna_now: () => performance.now(),
                urna_prof: (p, n) => { window.urnaProf = JSON.parse(str(p, n)); },
                urna_query: (kp, kn, p, cap) => put(enc.encode(new URLSearchParams(location.search).get(str(kp, kn)) || ""), p, cap),
                urna_store_get: (kp, kn, p, cap) => { try { return put(enc.encode(localStorage.getItem(str(kp, kn)) || ""), p, cap); } catch (e) { return 0; } },
                urna_store_set: (kp, kn, vp, vn) => { try { localStorage.setItem(str(kp, kn), str(vp, vn)); } catch (e) { } },
                urna_query_name: (p, cap) => put(enc.encode((new URLSearchParams(location.search).get("nome") || "").trim()), p, cap),
                urna_prompt: (mp, mn, p, cap) => {
                    if (pendingPrompt === null) {
                        if (document.exitPointerLock) document.exitPointerLock();
                        pendingPrompt = enc.encode(window.prompt(str(mp, mn), "") || "");
                    }
                    if (!pendingPrompt.length) { pendingPrompt = null; return 0; }
                    const r = put(pendingPrompt, p, cap);
                    if (r > 0) pendingPrompt = null;
                    return r;
                },
            });
        },
    });
})();
