// Cerebro unico de toda chamada de IA: OpenAI se tiver OPENAI_API_KEY, senao Workers AI (binding AI, gratis).
// Fila com prioridade (0 = pedido pago, 1 = jogador, 2 = fundo), limite por minuto e orcamento diario de
// "neurons" (cota gratis do Workers AI ~10k/dia). Retorna objeto JSON ou null (quem chama tem fallback).

const PER_MIN = 8;
const MAX_WAIT = 16;
const NEURON_DAY = 8000;
const MODELS = {
    big: "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
    small: "@cf/meta/llama-3.1-8b-instruct-fp8",
};
// neurons por 1M tokens (entrada, saida), estimativa conservadora da tabela da Cloudflare
const RATES = {
    "@cf/meta/llama-3.3-70b-instruct-fp8-fast": [26668, 204805],
    "@cf/meta/llama-3.1-8b-instruct-fp8": [14000, 35000],
};

/// Acha o primeiro objeto JSON balanceado no texto (ignora ```json, prosa, <think>, virgula sobrando).
export function parseJson(x) {
    if (x && typeof x === "object") return x;
    const s = String(x ?? "").replace(/<think>[\s\S]*?<\/think>/g, "");
    for (let i = s.indexOf("{"); i >= 0; i = s.indexOf("{", i + 1)) {
        let depth = 0, str = false, esc = false;
        for (let j = i; j < s.length; j++) {
            const ch = s[j];
            if (str) {
                if (esc) esc = false;
                else if (ch === "\\") esc = true;
                else if (ch === '"') str = false;
                continue;
            }
            if (ch === '"') str = true;
            else if (ch === "{") depth++;
            else if (ch === "}" && --depth === 0) {
                const raw = s.slice(i, j + 1);
                for (const t of [raw, raw.replace(/,\s*([}\]])/g, "$1")]) {
                    try {
                        const o = JSON.parse(t);
                        if (o && typeof o === "object") return o;
                    } catch (e) { }
                }
                break;
            }
        }
    }
    return null;
}

export class Brain {
    constructor(env, storage) {
        this.env = env;
        this.storage = storage;
        this.stamps = [];
        this.wait = [];
        this.seq = 0;
        this.timer = null;
        this.day = null;
    }

    slot(prio) {
        if (this.wait.length >= MAX_WAIT && prio > 0) return Promise.resolve(false);
        return new Promise((res) => {
            this.wait.push({ prio, seq: this.seq++, res });
            this.wait.sort((a, b) => a.prio - b.prio || a.seq - b.seq);
            this.pump();
        });
    }

    pump() {
        const now = Date.now();
        this.stamps = this.stamps.filter((t) => now - t < 60000);
        while (this.wait.length && this.stamps.length < PER_MIN) {
            this.stamps.push(now);
            this.wait.shift().res(true);
        }
        if (this.wait.length && !this.timer) {
            this.timer = setTimeout(() => {
                this.timer = null;
                this.pump();
            }, 60000 - (now - this.stamps[0]) + 50);
        }
    }

    async budget() {
        const today = new Date().toISOString().slice(0, 10);
        if (!this.day) this.day = (await this.storage.get("brain")) || { d: today, n: 0 };
        if (this.day.d !== today) this.day = { d: today, n: 0 };
        return this.day;
    }

    /// tier: "big" (construir/moderar) ou "small" (decisoes de fundo).
    async ask(system, user, prio = 1, tier = "big") {
        if (!(await this.slot(prio))) return null;
        const messages = [{ role: "system", content: system }, { role: "user", content: user }];
        if (this.env.OPENAI_API_KEY) {
            try {
                const r = await fetch("https://api.openai.com/v1/chat/completions", {
                    method: "POST",
                    headers: { "content-type": "application/json", authorization: `Bearer ${this.env.OPENAI_API_KEY}` },
                    body: JSON.stringify({ model: this.env.OPENAI_MODEL || "gpt-4.1-mini", response_format: { type: "json_object" }, messages }),
                });
                if (r.ok) {
                    const out = parseJson((await r.json()).choices?.[0]?.message?.content);
                    if (out) return out;
                }
            } catch (e) { }
        }
        if (!this.env.AI) return null;
        const day = await this.budget();
        if (day.n >= NEURON_DAY) return null;
        const model = (tier === "small" ? this.env.AI_MODEL_SMALL : this.env.AI_MODEL) || MODELS[tier] || MODELS.big;
        try {
            const r = await this.env.AI.run(model, { messages, max_tokens: tier === "small" ? 700 : 1400, temperature: 0.6 });
            const [ri, ro] = RATES[model] || RATES[MODELS.big];
            const text = typeof r?.response === "string" ? r.response : JSON.stringify(r?.response ?? "");
            const tin = r?.usage?.prompt_tokens ?? (system.length + user.length) / 3.5;
            const tout = r?.usage?.completion_tokens ?? text.length / 3.5;
            day.n += (tin * ri + tout * ro) / 1e6;
            this.storage.put("brain", day);
            return parseJson(r?.response);
        } catch (e) {
            return null;
        }
    }
}
