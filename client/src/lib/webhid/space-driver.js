// @ts-check
const deviceFilter = { vendorId: 0x046d };
const requestParams = { filters: [deviceFilter] };

// SpaceDriver
// Tested with Spaceball 5000 (USB) and SpaceExplorer
// but should work with other devices in the same range.
//
// Reads x, y, z, rx, ry, rz and buttons
// Dispatches to listeners via CustomEvents
//
// TODO:
//   - Support multiple devices connected in parallel
//   - Support sending settings (e.g. control LEDs) to devices.

export const SpaceDriver = new class extends EventTarget {
    #device // Just allow one device, for now

    constructor() {
        super();

        this.handleInputReport = this.handleInputReport.bind(this);

        // See if a paired device is already connected
        navigator.hid.getDevices().then((devices) => {
            devices.filter(d => d.vendorId === deviceFilter.vendorId).forEach(this.openDevice.bind(this));
        });

        navigator.hid.addEventListener('disconnect', evt => {
            const device = evt.device;
            console.log('disconnected', device);
            if (device === this.#device) {
                this.disconnect();
            }
        });

    }

    openDevice(device) {
        this.disconnect(); // If another device is connected - close it

        device.open().then(() => {
            console.log('Opened device: ' + device.productName);
            device.addEventListener('inputreport', this.handleInputReport);
            this.#device = device;
            this.dispatchEvent(new CustomEvent('connect', {detail: { device }}));
        });
    }

    disconnect() {
        this.#device?.close();
        this.#device = undefined;
        this.dispatchEvent(new Event('disconnect'));
    }

    scan() {
        navigator.hid.requestDevice(requestParams).then(devices => {
            if (devices.length == 0) return;
            this.openDevice(devices[0]);
        });
    }

    handleInputReport(e) {
        switch(e.reportId) {
            case 1: // x, y, z
            this.handleTranslation(new Int16Array(e.data.buffer));
            break;
            case 2: // yaw, pitch, roll
            this.handleRotation(new Int16Array(e.data.buffer));
            break;
            case 3: // buttons
            this.handleButtons(new Uint16Array(e.data.buffer)[0]);
            break;
        }
    }

    handleTranslation(val) {
        this.dispatchEvent(new CustomEvent('translate', {
            detail: {
                x: val[0],
                y: val[1],
                z: val[2]
            }
        }));
    }

    handleRotation(val) {
        this.dispatchEvent(new CustomEvent('rotate', {
            detail: {
                rx: -val[0],
                ry: -val[1],
                rz: val[2]
            }
        }));
    }

    handleButtons(val) {
        const buttons = [];
        for(let i=0;i<16;i++) {
            if(val & (1<<i)) buttons.push(`[${(i+1).toString(16).toUpperCase()}]`);
        }

        this.dispatchEvent(new CustomEvent('buttons', {
            detail: { buttons }
        }));
    }

}

// TODO: Add to class logic when working (note: 'connect' doesn't seem to work properly, Linux/Chrome 89)
navigator.hid.addEventListener('connect', evt => console.log('connect', evt.device));
