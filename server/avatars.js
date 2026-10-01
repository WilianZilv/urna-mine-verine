// Avatares embutidos (pacotes kind "avatar" do servidor): o passaporte SEMPRE leva um avatar. Sem skin escolhida,
// o jogador vai com o visual do personagem que esta usando no Urna. Ids "urna:<nome>" nao colidem com slugs de mods.
const V = "1.0.0";
const PX = 1.8 / 32; // Steve: 32 px de altura = 1.8 bloco

// Personagem do jogo ("ch" do hub.rs) -> visual
const LOOKS = {
    steve: { name: "Steve", skin: "#b98a6c", hair: "#4a2f1c", eye: "#4b3ca8", shirt: "#00a6a6", sleeve: "#00a6a6", pants: "#3a3fa6", shoes: "#5a5a5a" },
    skatista: { name: "Skatista", skin: "#c8956f", hair: "#2a1a10", eye: "#3a2a18", shirt: "#f2f2f2", sleeve: "#f2f2f2", pants: "#4a6fa8", shoes: "#1c1c1c", hat: "#d83a2e" },
    bandido: { name: "Bandido", skin: "#a87458", hair: "#1c1c1c", eye: "#2a1a10", shirt: "#e8e8e8", sleeve: "#2a2a2a", pants: "#2e3a5c", shoes: "#3a2414", jacket: "#2a2a2a" },
    portal: { name: "Arma de Portal", skin: "#d8a27e", hair: "#3a2414", eye: "#3a5a2a", shirt: "#f2f2f2", sleeve: "#f08a24", pants: "#f08a24", shoes: "#5c5c5c" },
};

function model(L) {
    const p = (n) => n * PX;
    const head = [
        { pos: [0, p(28), 0], size: [p(8), p(8), p(8)], color: L.skin },
        { pos: [0, p(31.5), 0], size: [p(8.4), p(1.4), p(8.4)], color: L.hat || L.hair },
        { pos: [0, p(29), -p(3.8)], size: [p(8.4), p(6), p(1)], color: L.hat || L.hair },
        { pos: [0, p(30.5), p(4.1)], size: [p(8.4), p(1.2), p(1)], color: L.hat || L.hair },
        { pos: [-p(2.5), p(27.5), p(4.1)], size: [p(2), p(1), p(1)], color: "#f2f2f2" },
        { pos: [p(2.5), p(27.5), p(4.1)], size: [p(2), p(1), p(1)], color: "#f2f2f2" },
        { pos: [-p(2), p(27.5), p(4.3)], size: [p(1), p(1), p(1)], color: L.eye },
        { pos: [p(2), p(27.5), p(4.3)], size: [p(1), p(1), p(1)], color: L.eye },
        { pos: [0, p(26.5), p(4.1)], size: [p(2), p(1), p(1)], color: "#8a5a3e" },
        { pos: [0, p(25.5), p(4.1)], size: [p(4), p(1), p(1)], color: "#5a3424" },
    ];
    if (L.hat) head.push({ pos: [0, p(32.6), 0], size: [p(7), p(1), p(7)], color: L.hat });
    const body = [{ pos: [0, p(18), 0], size: [p(8), p(12), p(4)], color: L.shirt }];
    if (L.jacket) body.push({ pos: [-p(2.6), p(18), p(0.2)], size: [p(2.8), p(12.2), p(4.2)], color: L.jacket }, { pos: [p(2.6), p(18), p(0.2)], size: [p(2.8), p(12.2), p(4.2)], color: L.jacket });
    const arm = (s) => [
        { pos: [s * p(6), p(21.5), 0], size: [p(4), p(5), p(4)], color: L.sleeve },
        { pos: [s * p(6), p(15.5), 0], size: [p(4), p(7), p(4)], color: L.skin },
    ];
    const leg = (s) => [
        { pos: [s * p(2), p(7), 0], size: [p(4), p(10), p(4)], color: L.pants },
        { pos: [s * p(2), p(1), p(0.2)], size: [p(4.2), p(2), p(4.4)], color: L.shoes },
    ];
    return {
        parts: [
            { name: "body", pivot: [0, p(12), 0], boxes: body },
            { name: "head", parent: "body", pivot: [0, p(24), 0], boxes: head },
            { name: "arm_l", parent: "body", pivot: [p(6), p(23), 0], boxes: arm(1) },
            { name: "arm_r", parent: "body", pivot: [-p(6), p(23), 0], boxes: arm(-1) },
            { name: "leg_l", pivot: [p(2), p(12), 0], boxes: leg(1) },
            { name: "leg_r", pivot: [-p(2), p(12), 0], boxes: leg(-1) },
        ],
    };
}

const swing = (a, l, body = 0) => ({ body: { rot: [body, 0, 0] }, leg_l: { rot: [l, 0, 0] }, leg_r: { rot: [-l, 0, 0] }, arm_l: { rot: [-a, 0, 0] }, arm_r: { rot: [a, 0, 0] } });
const ANIMS = {
    idle: { duration: 3, loop: true, keyframes: [{ t: 0, parts: {} }, { t: 1.5, parts: { head: { rot: [4, 8, 0] }, arm_l: { rot: [0, 0, -3] }, arm_r: { rot: [0, 0, 3] } } }, { t: 3, parts: {} }] },
    walk: { duration: 0.8, loop: true, keyframes: [{ t: 0, parts: swing(30, 30) }, { t: 0.4, parts: swing(-30, -30) }, { t: 0.8, parts: swing(30, 30) }] },
    run: { duration: 0.5, loop: true, keyframes: [{ t: 0, parts: swing(55, 55, 10) }, { t: 0.25, parts: swing(-55, -55, 10) }, { t: 0.5, parts: swing(55, 55, 10) }] },
    jump: { duration: 0.6, keyframes: [{ t: 0, parts: {} }, { t: 0.25, parts: { arm_l: { rot: [-150, 0, -10] }, arm_r: { rot: [-150, 0, 10] }, leg_l: { rot: [-20, 0, 0] }, leg_r: { rot: [15, 0, 0] } } }, { t: 0.6, parts: { arm_l: { rot: [-100, 0, -15] }, arm_r: { rot: [-100, 0, 15] } } }] },
    attack: { duration: 0.35, keyframes: [{ t: 0, parts: {} }, { t: 0.1, parts: { arm_r: { rot: [-100, 0, 0] }, body: { rot: [0, -12, 0] } } }, { t: 0.35, parts: {} }] },
    emote1: { duration: 1, loop: true, keyframes: [{ t: 0, parts: { arm_l: { rot: [0, 0, -150] } } }, { t: 0.5, parts: { arm_l: { rot: [0, 0, -115] }, head: { rot: [0, 10, 0] } } }, { t: 1, parts: { arm_l: { rot: [0, 0, -150] } } }] },
};

/// Pacotes crus (o mods.js valida e troca o manifest.id pelo "urna:<nome>").
export const BUILTIN_AVATARS = Object.fromEntries(Object.entries(LOOKS).map(([k, L]) => [`urna:${k}`, {
    manifest: { id: `urna-${k}`, kind: "avatar", name: L.name, version: V, author: "urna", description: `Visual padrao do personagem ${L.name} do Urna (quando o jogador nao escolheu skin).`, license: "CC0-1.0" },
    model: model(L),
    animations: ANIMS,
    avatar: { scale: 1, emotes: [{ anim: "emote1", name: "Acenar" }] },
}]));

/// "urna:<personagem>@versao" pro personagem do jogo (desconhecido -> steve).
export function builtinRef(ch) {
    const k = { niko: "bandido" }[ch] || ch;
    return `${BUILTIN_AVATARS[`urna:${k}`] ? `urna:${k}` : "urna:steve"}@${V}`;
}
