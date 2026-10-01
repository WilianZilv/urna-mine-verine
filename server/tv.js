// TV URNA NEWS: ancora IA resume o que rolou no mundo; telao de fachada (src/places/tv.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("tv", this.s)).

export class Tv {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
    }
}
