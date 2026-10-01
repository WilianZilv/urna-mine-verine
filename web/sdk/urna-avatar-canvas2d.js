/*! Urna avatar -> canvas 2D (Phaser, PixiJS, canvas puro). Doc: https://urna-mine-verine.wilianzilv.workers.dev/universe.txt
 *
 *   <script src="https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-avatar-canvas2d.js"></script>
 *   const av = UrnaAvatar2D.create(UrnaPortal.avatar(), { view: "side", size: 96 });  // null sem avatar
 *   // todo frame: pes em (x, y), altura h px, flip = olhando pra esquerda
 *   av.draw(ctx, x, y, h, { speed: 3 }, performance.now() / 1000, flip);
 *   // Phaser: const sheet = av.sheet("walk"); this.textures.addSpriteSheet("me-walk", sheet.canvas, { frameWidth: sheet.w, frameHeight: sheet.h });
 *
 * view: "side" (perfil, olhando pra direita), "front", "3q" (3/4) ou "top". Cada frame e renderizado UMA vez
 * (rasterizador de caixas em JS, ordem de pintor) e guardado num canvas offscreen; depois e so drawImage.
 */
(function (root) {
    "use strict";
    const D2R = Math.PI / 180;
    const VIEWS = { side: [Math.PI / 2 - 0.25, 0.18], front: [0, 0.12], "3q": [0.6, 0.3], top: [0, Math.PI / 2 - 0.05] };

    function pick(pkg, st) {
        const has = (a) => pkg.animations && pkg.animations[a];
        for (const a of [st.anim, st.dead && "death", st.attack && "attack", st.emote, st.air && (st.fly ? "fly" : "jump"), (st.speed || 0) > 4.5 && "run", ((st.speed || 0) > 0.2 || st.moving) && "walk", "idle"]) if (a && has(a)) return a;
        return null;
    }
    function pose(pkg, name, t) {
        const out = {}, a = name && pkg.animations && pkg.animations[name];
        if (!a || !a.keyframes.length) return out;
        const ks = a.keyframes, tt = a.loop ? ((t % a.duration) + a.duration) % a.duration : Math.min(t, a.duration);
        let k0 = ks[0], k1 = ks[0], w = 0;
        if (tt >= ks[ks.length - 1].t) k0 = k1 = ks[ks.length - 1];
        else if (tt > ks[0].t) for (let i = 0; i < ks.length - 1; i++) if (tt >= ks[i].t && tt < ks[i + 1].t) { k0 = ks[i]; k1 = ks[i + 1]; w = (tt - k0.t) / Math.max(1e-4, k1.t - k0.t); break; }
        for (const p of pkg.model.parts) {
            const a0 = k0.parts[p.name] || {}, a1 = k1.parts[p.name] || {};
            const l = (u, v) => [0, 1, 2].map((i) => (u ? u[i] : 0) * (1 - w) + (v ? v[i] : 0) * w);
            out[p.name] = { rot: l(a0.rot, a1.rot).map((x) => x * D2R), offset: l(a0.offset, a1.offset) };
        }
        return out;
    }

    // ------------------------------------------------ matriz 4x4 (coluna-major, igual glam/three)
    const I = () => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    function mul(a, b) {
        const o = new Array(16);
        for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) o[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
        return o;
    }
    const T = (x, y, z) => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1];
    const RX = (a) => { const c = Math.cos(a), s = Math.sin(a); return [1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1]; };
    const RY = (a) => { const c = Math.cos(a), s = Math.sin(a); return [c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]; };
    const RZ = (a) => { const c = Math.cos(a), s = Math.sin(a); return [c, s, 0, 0, -s, c, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]; };
    const ap = (m, v) => [m[0] * v[0] + m[4] * v[1] + m[8] * v[2] + m[12], m[1] * v[0] + m[5] * v[1] + m[9] * v[2] + m[13], m[2] * v[0] + m[6] * v[1] + m[10] * v[2] + m[14]];
    const FACES = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
    const shade = (hex, k) => {
        const n = parseInt(hex.slice(1), 16);
        const f = (v) => Math.max(0, Math.min(255, Math.round(v * k)));
        return `rgb(${f(n >> 16)},${f((n >> 8) & 255)},${f(n & 255)})`;
    };

    /// Rasteriza o modelo numa pose: poligonos das faces visiveis em ordem de pintor.
    function render(ctx, pkg, ps, view, ox, oy, ppu) {
        const [yaw, pitch] = view;
        const V = mul(RX(pitch), RY(yaw));
        const sc = (pkg.avatar && pkg.avatar.scale) || 1;
        const root = mul(V, [sc, 0, 0, 0, 0, sc, 0, 0, 0, 0, sc, 0, 0, 0, 0, 1]);
        const mats = {}, polys = [];
        const L = [-0.35, 0.75, 0.55];
        for (const p of pkg.model.parts) {
            const q = ps[p.name] || { rot: [0, 0, 0], offset: [0, 0, 0] };
            const pv = p.pivot || [0, 0, 0];
            const par = p.parent ? mats[p.parent] : root;
            const m = mul(mul(mul(par, T(pv[0] + q.offset[0], pv[1] + q.offset[1], pv[2] + q.offset[2])), mul(RY(q.rot[1]), mul(RX(q.rot[0]), RZ(q.rot[2])))), T(-pv[0], -pv[1], -pv[2]));
            mats[p.name] = m;
            for (const b of p.boxes) {
                const h = b.size.map((s) => s / 2);
                const cs = [];
                for (let i = 0; i < 8; i++) cs.push(ap(m, [b.pos[0] + (i & 4 ? h[0] : -h[0]), b.pos[1] + (i & 2 ? h[1] : -h[1]), b.pos[2] + (i & 1 ? h[2] : -h[2])]));
                for (const f of FACES) {
                    const [a, bb, c] = [cs[f[0]], cs[f[1]], cs[f[2]]];
                    const u = [bb[0] - a[0], bb[1] - a[1], bb[2] - a[2]], v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    const n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                    const len = Math.hypot(...n) || 1;
                    if (n[2] / len <= 0.001) continue;
                    const lit = b.glow ? 1.15 : 0.55 + 0.5 * Math.max(0, (n[0] * L[0] + n[1] * L[1] + n[2] * L[2]) / len);
                    polys.push({ z: f.reduce((s, i) => s + cs[i][2], 0) / 4, pts: f.map((i) => cs[i]), col: shade(b.color, lit) });
                }
            }
        }
        polys.sort((a, b) => a.z - b.z);
        for (const p of polys) {
            ctx.beginPath();
            p.pts.forEach((v, i) => (i ? ctx.lineTo : ctx.moveTo).call(ctx, ox + v[0] * ppu, oy - v[1] * ppu));
            ctx.closePath();
            ctx.fillStyle = p.col;
            ctx.fill();
            ctx.strokeStyle = p.col;
            ctx.lineWidth = 0.6;
            ctx.stroke();
        }
    }

    function create(pkg, opts = {}) {
        if (!pkg || !pkg.model || typeof document === "undefined") return null;
        const view = VIEWS[opts.view] || VIEWS.side;
        const size = Math.max(16, Math.min(512, opts.size || 96));
        const fps = opts.fps || 12;
        const hgt = ((pkg.avatar && pkg.avatar.height) || 1.8) * 1.3;
        const W = size, H = size, ppu = size / hgt, ox = W / 2, oy = H * 0.88;
        const cache = new Map();
        const frame = (anim, i) => {
            const k = `${anim}#${i}`;
            if (!cache.has(k)) {
                const cv = document.createElement("canvas");
                cv.width = W;
                cv.height = H;
                render(cv.getContext("2d"), pkg, pose(pkg, anim, i / fps), view, ox, oy, ppu);
                cache.set(k, cv);
            }
            return cache.get(k);
        };
        const count = (anim) => {
            const a = anim && pkg.animations[anim];
            return a ? Math.max(1, Math.min(48, Math.round(a.duration * fps))) : 1;
        };
        let cur = null, t0 = 0;
        return {
            width: W, height: H, emotes: (pkg.avatar && pkg.avatar.emotes) || [],
            /** Desenha com os pes em (x, y) e altura do avatar = h px (padrao = size). t em segundos. */
            draw(ctx, x, y, h, state = {}, t = 0, flip = false) {
                const anim = pick(pkg, state);
                if (anim !== cur) { cur = anim; t0 = t; }
                const a = anim && pkg.animations[anim], n = count(anim);
                let i = Math.floor((t - t0) * fps);
                i = a && a.loop === false ? Math.min(i, n - 1) : ((i % n) + n) % n;
                const img = frame(anim, i), k = (h || hgt / 1.3 * ppu) / (hgt / 1.3 * ppu);
                ctx.save();
                ctx.translate(x, y);
                if (flip) ctx.scale(-1, 1);
                ctx.drawImage(img, -ox * k, -oy * k, W * k, H * k);
                ctx.restore();
                return anim;
            },
            /** Sprite sheet horizontal da animacao: {canvas, w, h, frames}. */
            sheet(anim) {
                const n = count(anim), cv = document.createElement("canvas");
                cv.width = W * n;
                cv.height = H;
                const g = cv.getContext("2d");
                for (let i = 0; i < n; i++) g.drawImage(frame(anim, i), i * W, 0);
                return { canvas: cv, w: W, h: H, frames: n };
            },
            /** Retrato parado (ex: HUD/lista de jogadores). */
            still() { return frame(pick(pkg, {}), 0); },
        };
    }

    const UrnaAvatar2D = { create, render, pick, pose };
    root.UrnaAvatar2D = UrnaAvatar2D;
    if (typeof module === "object" && module.exports) module.exports = UrnaAvatar2D;
})(typeof window !== "undefined" ? window : this);
