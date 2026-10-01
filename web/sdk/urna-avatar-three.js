/*! Urna avatar -> three.js. Doc: https://urna-mine-verine.wilianzilv.workers.dev/universe.txt
 *
 *   <script src="https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-avatar-three.js"></script>
 *   const av = UrnaAvatarThree.build(UrnaPortal.avatar(), THREE);   // null se nao tiver avatar
 *   scene.add(av.group);                                             // pes em y=0, olha pra +Z, 1 unidade = 1 bloco
 *   // todo frame:
 *   av.update(dt, { speed: 3.2, air: false });   // ou { anim: "emote1" } / { attack: true } / { dead: true }
 *   av.group.position.set(x, y, z); av.group.rotation.y = yaw;
 *
 * Nada de rede aqui: so geometria (caixas) + animacao por keyframes do pacote validado pelo servidor.
 */
(function (root) {
    "use strict";
    const D2R = Math.PI / 180;

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

    function build(pkg, THREE, opts = {}) {
        if (!pkg || !pkg.model || !THREE) return null;
        const group = new THREE.Group();
        const body = new THREE.Group();
        body.scale.setScalar((pkg.avatar && pkg.avatar.scale) || 1);
        group.add(body);
        const geo = new THREE.BoxGeometry(1, 1, 1);
        const mats = new Map();
        const mat = (c, glow) => {
            const k = c + (glow ? "g" : "");
            if (!mats.has(k)) mats.set(k, glow ? new THREE.MeshBasicMaterial({ color: c }) : new (opts.material || THREE.MeshLambertMaterial)({ color: c }));
            return mats.get(k);
        };
        const nodes = {}, base = {};
        for (const p of pkg.model.parts) {
            const pv = p.pivot || [0, 0, 0], pp = p.parent ? pkg.model.parts.find((q) => q.name === p.parent).pivot || [0, 0, 0] : [0, 0, 0];
            const n = new THREE.Group();
            n.name = p.name;
            n.rotation.order = "YXZ";
            base[p.name] = [pv[0] - pp[0], pv[1] - pp[1], pv[2] - pp[2]];
            n.position.set(...base[p.name]);
            for (const b of p.boxes) {
                const m = new THREE.Mesh(geo, mat(b.color, b.glow));
                m.position.set(b.pos[0] - pv[0], b.pos[1] - pv[1], b.pos[2] - pv[2]);
                m.scale.set(...b.size);
                m.castShadow = !!opts.shadows;
                n.add(m);
            }
            (p.parent ? nodes[p.parent] : body).add(n);
            nodes[p.name] = n;
        }
        let t = 0, cur = null;
        return {
            group,
            nodes,
            height: (pkg.avatar && pkg.avatar.height) || 1.8,
            emotes: (pkg.avatar && pkg.avatar.emotes) || [],
            /** dt em segundos; state: {anim?, speed?, moving?, air?, fly?, attack?, emote?, dead?}. Retorna a animacao tocando. */
            update(dt, state = {}) {
                const a = pick(pkg, state);
                if (a !== cur) { cur = a; t = 0; }
                t += dt;
                const ps = pose(pkg, a, t);
                for (const [name, n] of Object.entries(nodes)) {
                    const q = ps[name], b = base[name];
                    n.position.set(b[0] + (q ? q.offset[0] : 0), b[1] + (q ? q.offset[1] : 0), b[2] + (q ? q.offset[2] : 0));
                    n.rotation.set(q ? q.rot[0] : 0, q ? q.rot[1] : 0, q ? q.rot[2] : 0);
                }
                return a;
            },
            dispose() { geo.dispose(); for (const m of mats.values()) m.dispose(); },
        };
    }

    const UrnaAvatarThree = { build, pick, pose };
    root.UrnaAvatarThree = UrnaAvatarThree;
    if (typeof module === "object" && module.exports) module.exports = UrnaAvatarThree;
})(typeof window !== "undefined" ? window : this);
