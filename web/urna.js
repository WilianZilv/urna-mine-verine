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
        const track = c.captureStream(30).getVideoTracks()[0];
        const draw = () => {
            if (track.readyState !== "live") return;
            const s = Math.min(1, 1280 / gl.width), w = gl.width * s | 0, h = gl.height * s | 0;
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
        if (ctx.state === "running" && ctx.createMediaStreamDestination) {
            if (!audioTap) { audioTap = ctx.createMediaStreamDestination(); master.connect(audioTap); }
            t.push(audioTap.stream.getAudioTracks()[0]);
        }
        return t;
    }
    function startSeg() {
        const seg = { chunks: [], t0: performance.now() };
        seg.rec = new MediaRecorder(new MediaStream(clipTracks()), { mimeType: clipMime, videoBitsPerSecond: weakDevice ? 1500000 : 2500000 });
        seg.rec.ondataavailable = (e) => { if (e.data.size) seg.chunks.push(e.data); };
        seg.rec.start(1000);
        return seg;
    }
    function dropSeg(seg) { if (seg && seg.rec.state !== "inactive") { seg.rec.ondataavailable = null; seg.rec.stop(); } }
    const stopSeg = (seg) => new Promise((r) => { seg.rec.onstop = () => r(seg); seg.rec.stop(); });
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
    function recDot(on) { const d = document.getElementById("rec"); if (d) d.style.display = on ? "block" : "none"; }
    function saveSeg(seg) {
        const blob = new Blob(seg.chunks, { type: clipMime.split(";")[0] });
        if (blob.size < 1000) { clipMsgs.push("CLIPE: deu ruim, video saiu vazio"); return; }
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
                    if (e.data === 1) { const d = player.getVideoData(); ytTitle = (d && d.title) || ""; }
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
