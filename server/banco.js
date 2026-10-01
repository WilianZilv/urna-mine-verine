// Banco Central da Vila: ranking, ledger, caixa eletronico e poupanca FICTICIA paga pelo cofre (src/places/banco.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("banco", this.s)).

export class Banco {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
    }
}
