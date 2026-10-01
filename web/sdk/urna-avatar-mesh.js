/*! Urna avatar core: pacote de avatar (mod kind "avatar") -> descricao generica de malha + pose animada.
 * Pra qualquer engine (Godot, Unity, Babylon, PlayCanvas...). Doc: https://urna-mine-verine.wilianzilv.workers.dev/universe.txt
 *
 *   const mesh = UrnaAvatarMesh.flatten(pkg);  // {scale, height, parts:[{name, parent, origin:[x,y,z], boxes:[{center, size, color, glow}]}]}
 *   const pose = UrnaAvatarMesh.pose(pkg, UrnaAvatarMesh.pick(pkg, {speed: 3}), t);  // {part: {rot:[x,y,z] rad, offset:[x,y,z]}}
 *
 * Montagem: um no por parte. origin = pivo relativo ao pivo do pai (ou ao chao, sem pai). Caixas relativas ao pivo da parte.
 * A cada frame: no.position = origin + offset; no.rotation = rot com ordem Y, depois X, depois Z (Euler "YXZ").
 * Unidades: 1 = 1 bloco (~1 m). Y pra cima, o modelo olha pra +Z, pes em y=0. Escala final = mesh.scale.
 */
(function (root) {
    "use strict";
    const D2R = Math.PI / 180;

    function flatten(pkg) {
        const parts = (pkg && pkg.model && pkg.model.parts) || [];
        const piv = Object.fromEntries(parts.map((p) => [p.name, p.pivot || [0, 0, 0]]));
        return {
            scale: (pkg.avatar && pkg.avatar.scale) || 1,
            height: (pkg.avatar && pkg.avatar.height) || 1.8,
            parts: parts.map((p) => {
                const pv = p.pivot || [0, 0, 0], pp = p.parent ? piv[p.parent] : [0, 0, 0];
                return {
                    name: p.name,
                    parent: p.parent || null,
                    origin: [pv[0] - pp[0], pv[1] - pp[1], pv[2] - pp[2]],
                    boxes: p.boxes.map((b) => ({ center: [b.pos[0] - pv[0], b.pos[1] - pv[1], b.pos[2] - pv[2]], size: b.size, color: b.color, glow: !!b.glow })),
                };
            }),
        };
    }

    /// Escolhe a animacao pelo estado: {anim} explicito, ou dead/attack/air/speed. Retorna nome existente ou null.
    function pick(pkg, st = {}) {
        const has = (a) => pkg && pkg.animations && pkg.animations[a];
        const order = [st.anim, st.dead && "death", st.attack && "attack", st.emote, st.air && (st.fly ? "fly" : "jump"), (st.speed || 0) > 4.5 && "run", ((st.speed || 0) > 0.2 || st.moving) && "walk", "idle"];
        for (const a of order) if (a && has(a)) return a;
        return null;
    }

    function pose(pkg, name, t) {
        const out = {};
        const a = name && pkg.animations && pkg.animations[name];
        if (!a || !a.keyframes.length) return out;
        const ks = a.keyframes;
        const tt = a.loop ? ((t % a.duration) + a.duration) % a.duration : Math.min(t, a.duration);
        let k0 = ks[0], k1 = ks[0], w = 0;
        if (tt >= ks[ks.length - 1].t) k0 = k1 = ks[ks.length - 1];
        else if (tt > ks[0].t) {
            for (let i = 0; i < ks.length - 1; i++) if (tt >= ks[i].t && tt < ks[i + 1].t) { k0 = ks[i]; k1 = ks[i + 1]; w = (tt - k0.t) / Math.max(1e-4, k1.t - k0.t); break; }
        }
        for (const p of pkg.model.parts) {
            const a0 = k0.parts[p.name] || {}, a1 = k1.parts[p.name] || {};
            const lerp = (u, v) => [0, 1, 2].map((i) => ((u ? u[i] : 0) * (1 - w) + (v ? v[i] : 0) * w));
            out[p.name] = { rot: lerp(a0.rot, a1.rot).map((x) => x * D2R), offset: lerp(a0.offset, a1.offset) };
        }
        return out;
    }

    const UrnaAvatarMesh = { flatten, pick, pose };
    root.UrnaAvatarMesh = UrnaAvatarMesh;
    if (typeof module === "object" && module.exports) module.exports = UrnaAvatarMesh;
})(typeof window !== "undefined" ? window : this);
