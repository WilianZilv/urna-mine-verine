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
        if (ytReady) player.loadVideoById(id);
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
                    src.connect(g).connect(ctx.destination);
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
