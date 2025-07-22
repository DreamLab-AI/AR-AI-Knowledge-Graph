// @ts-check
import { Demo3DObj } from './demo-3dobj.js';
import { SpaceDriver } from './space-driver.js';

export class MainApp extends HTMLElement {
    /** @type {Demo3dobj} */ #obj
    #cells

    constructor() {
        super();

        this.#cells = [];

        this.handleTranslate = this.handleTranslate.bind(this);
        this.handleRotate = this.handleRotate.bind(this);
        this.handleButtons = this.handleButtons.bind(this);
        this.handleConnect = this.handleConnect.bind(this);
        this.handleDisconnect = this.handleDisconnect.bind(this);
    }

    connectedCallback() {
        this.innerHTML = `
        <style>
        #list {
            display: grid;
            grid-template-columns: 1fr 1fr;
            width: 300px;
        }

        </style>

        <button id='connect'>CONNECT</button>
        <h2>Status: <span id='status'> - </span></h2>
        <div id='list'></div>
        <demo-3dobj></demo-3dobj>
        `;


        this.#obj = this.querySelector('demo-3dobj');
        this.querySelector('#connect').addEventListener('click', this.doScan);

        this._initList();

        SpaceDriver.addEventListener('translate', this.handleTranslate);
        SpaceDriver.addEventListener('rotate', this.handleRotate);
        SpaceDriver.addEventListener('buttons', this.handleButtons);
        SpaceDriver.addEventListener('connect', this.handleConnect);
        SpaceDriver.addEventListener('disconnect', this.handleDisconnect);
    }

    disconnectedCallback() {
        SpaceDriver.removeEventListener('translate', this.handleTranslate);
        SpaceDriver.removeEventListener('rotate', this.handleRotate);
        SpaceDriver.removeEventListener('buttons', this.handleButtons);
        SpaceDriver.removeEventListener('connect', this.handleConnect);
        SpaceDriver.removeEventListener('disconnect', this.handleDisconnect);
    }

    _initList() {
        const list = this.querySelector('#list');
        const labels = ['X', 'Y', 'Z', 'Pitch', 'Roll', 'Yaw', 'Buttons'];

        labels.forEach(l => {
            const label = document.createElement('span');
            label.classList.add('label');
            label.innerText = l;

            const value = document.createElement('span');
            value.classList.add('value');
            value.innerText = `-`;
            this.#cells.push(value);

            list.append(label, value);
        });
    }

    setStatus(str) {
        this.querySelector('#status').innerHTML = str;
    }

    setCellValue(i, val) {
        this.#cells[i].innerText = val;
    }

    doScan() {
        SpaceDriver.scan();
    }

    handleTranslate(/** @type {CustomEvent} */ evt) {
        const {x, y, z} = evt.detail;
        this.#obj.setTranslation(x, y, -z);
        this.setCellValue(0, x);
        this.setCellValue(1, y);
        this.setCellValue(2, z);
    }

    handleRotate(/** @type {CustomEvent} */ evt) {
        const {rx, ry, rz} = evt.detail;
        this.#obj.setRotation(rx/5, ry/5, rz/5);
        this.setCellValue(3, rx);
        this.setCellValue(4, ry);
        this.setCellValue(5, rz);
    }

    handleButtons(/** @type {CustomEvent} */ evt) {
        const {buttons} = evt.detail;
        this.setCellValue(6, buttons.join(', '));
    }

    handleConnect(/** @type {CustomEvent} */ evt) {
        const {device} = evt.detail;
        this.setStatus(`${device.productName} connected`);
    }

    handleDisconnect() {
        this.setStatus(` - `);
    }
}
customElements.define('main-app', MainApp);
