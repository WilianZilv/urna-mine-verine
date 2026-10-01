// Terminal Interdimensional: painel de partidas (vila + jogos do Hub) e viagens rapidas (src/places/terminal.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("terminal", this.s)).

export class Terminal {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
    }
}
