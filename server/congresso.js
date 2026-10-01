// Congresso da Vila: jogadores votam leis que mudam o jogo por alguns minutos (src/places/congresso.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("congresso", this.s)).

export class Congresso {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
    }
}
