---
title: Reviving legacy hardware with Web HID
published: true
description: Web HID makes it easy to web enable (legacy) devices.
tags:
cover_image: https://dev-to-uploads.s3.amazonaws.com/uploads/articles/nyg9s8q7p3n6dbxm277c.jpg
---

## Finding gold!
For many years, I've been saving different odd hardware devices, that I would find use for SomeDay(TM).

In reality, that day rarely comes, so a few months back, I decided to get rid of all the unused stuff.

One item I initially put in the 'out' pile was a 3Dconnexion Spaceball 5000, because surely there would be little to no support for it in any OS.

However, I tried to connect it to my Ubuntu machine to see what would happen.

![3DConnection Spaceball 5000](https://dev-to-uploads.s3.amazonaws.com/uploads/articles/nyg9s8q7p3n6dbxm277c.jpg)

Running `lsusb` gives us the following:

```
ID 046d:c621 Logitech, Inc. 3Dconnexion Spaceball 5000
```

... adding the `-v` switch, tells us that there is one exposed HID interface:

```
...
        bInterfaceClass         3 Human Interface Device
...
```

NOTE:  Using (Ubuntu) Linux, in order to get access from user space, add the following, using the USB Vendor ID returned from `lsusb` to a udev rules file (e.g. `/etc/udev/rules/50-webhid.rules`):

```
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="046d", MODE:="0666", GROUP="input"
```

And run `sudo udevadm control --reload-rules && sudo udevadm trigger`

On other systems, this is not necessary.


## WebHID
Looking at [the Fugu API tracker](https://fugu-tracker.web.app/), we see that [WebHID](https://wicg.github.io/webhid/) was released in Chrome M89, so let's try to hook it up and see what kind of data the Spaceball sends out.

We will start with something very simple, just to read all the data coming from the device.

_(Remember to run this from a local web server and to do the `requestDevice` call based on a user gesture, e.g. `click` event)_

```html
<button id="scan">SCAN</button>
<script>
function scan() {
    navigator.hid.requestDevice({filters: [{ vendorId: 0x046d }]}).then(devices => {
        if (!devices.length) return;

        const device = devices[0];

        device.open().then(() => {
            console.log('Opened device: ' + device.productName);
            device.addEventListener('inputreport', e => {
                console.log('Report ID', e.reportId);
                console.log('Data', new Int8Array(e.data.buffer));
            });
        });
    });
}

document.querySelector('#scan').addEventListener('click', scan);
</script>
```

Running this in Chrome and clicking the `SCAN` button with the device attached should show a dialog, where we can now select the Spaceball 5000:

![WebHID device selector](https://dev-to-uploads.s3.amazonaws.com/uploads/articles/rqzfx5u692w6jtlgzasd.png)

After a successful connection, you should see the following in the console:

`Opened device: 3Dconnexion SpaceBall 5000 USB`

YAY!

## All the data!
Manipulating the Spaceball sends a lot of data packets of 6 bytes to the console, which seems to be pairs with report ID 1 and 2:

```
Report ID 1
Data Int8Array(6) [101, -1, -2, -1, 95, 0]
Report ID 2
Data Int8Array(6) [-27, 0, 35, 1, -126, -1]
```

After searching a bit for a protocol description, I read a small [post](https://forum.3dconnexion.com/viewtopic.php?p=11239#p11239) from 2006, that the values are 16-bit signed X, Y, Z translation values, followed by the same for rotation.

Let's make a small change to the code:

```javascript
...
device.addEventListener('inputreport', e => {
    // First attempt: Print all the data
    // console.log('Report ID', e.reportId);
    // console.log('Data', new Int8Array(e.data.buffer));

    // Second attempt: extract translation and rotation:
    if (e.reportId === 1) console.log('T', new Int16Array(e.data.buffer));
    if (e.reportId === 2) console.log('R', new Int16Array(e.data.buffer));
});
...
```

Now the data starts to make a bit more sense:

```
T Int16Array(3) [33, -18, 132]
R Int16Array(3) [-230, 67, 0]
T Int16Array(3) [39, -15, 124]
R Int16Array(3) [-208, 67, 0]
```

and when releasing the Spaceball, it seems to go to zero:

```
T Int16Array(3) [0, 0, 0]
R Int16Array(3) [0, 0, 0]
```


## Making a simple driver
Now that all the basics are in place, let's write a small web driver for the device.

Extending [EventTarget](https://developer.mozilla.org/en-US/docs/Web/API/EventTarget/EventTarget) gives the driver the ability to send the parsed packets on to any listner as proper events.

```javascript
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
}

```

## Visualizng with Web Components and CSS3D
Just writing numbers in the console is no fun, and as we are using a modern browser with native CSS3D and Web Component support, why not make a silly 3D object in a component, we can manipulate with the Spaceball:

```javascript
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
```

All that is left is to combine the `SpaceDriver` and `demo-3dobj` to see the direct manipulation of a 3D object using the WebHID connected Spaceball 5000:

![WebHID 3D Object Manipulation](https://dev-to-uploads.s3.amazonaws.com/uploads/articles/tr9d33bald3f8gbrtsna.png)


## Live demo and source code on GitHub

**[Try it out here!](https://larsgk.github.io/webhid-space)**

[source](https://github.com/larsgk/webhid-space)

ENJOY!