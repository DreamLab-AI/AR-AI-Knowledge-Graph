// @ts-check
export class Demo3DObj extends HTMLElement {
    #objtranslate
    #objrotate
    #obj

    constructor() {
        super();
        this.#objtranslate = '';
        this.#objrotate = '';
    }

    connectedCallback() {
        this.innerHTML = `
        <style>
        .scene {
            width: 200px;
            height: 200px;
            margin: 200px;
            perspective: 500px;
        }

        .obj {
            width: 200px;
            height: 200px;
            position: relative;
            transform-style: preserve-3d;
            transform: translateZ(-1000px);
            transition: transform 100ms;
        }

        .plane {
            position: absolute;
            width: 200px;
            height: 200px;
            border: 5px solid black;
            border-radius: 50%;
        }

        .red {
            background: rgba(255,0,0,0.5);
            transform: rotateY(-90deg)
        }

        .green {
            background: rgba(0,255,0,0.5);
            transform: rotateX( 90deg)
        }

        .blue {
            background: rgba(0,0,255,0.5);
        }
        </style>

        <div class="scene">
            <div class="obj">
                <div class="plane red"></div>
                <div class="plane green"></div>
                <div class="plane blue"></div>
            </div>
        </div>
        `;

        this.#obj = this.querySelector('.obj');
    }

    setTranslation(x, y, z) {
        this.#objtranslate = `translateX(${x}px) translateY(${y}px) translateZ(${z}px) `;
        this.#obj.style.transform = this.#objtranslate + this.#objrotate;
    }

    setRotation(rx, ry, rz) {
        this.#objrotate = `rotateX(${rx}deg) rotateY(${ry}deg) rotateZ(${rz}deg) `;
        this.#obj.style.transform = this.#objtranslate + this.#objrotate;
    }
}
customElements.define('demo-3dobj', Demo3DObj);
