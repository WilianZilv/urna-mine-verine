// Bolsa de Valores da Vila: acoes FICTICIAS que reagem a arena e a cidade; contraparte e o cofre da IA (src/places/bolsa.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("bolsa", this.s)).

export class Bolsa {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
    }
}
