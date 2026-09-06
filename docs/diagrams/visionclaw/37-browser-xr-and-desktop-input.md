---
id: VC-37
title: Browser XR surface and desktop spatial input
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
  - docs/XR-client.md
adrs: [ADR-2032, ADR-2081]
sources:
  - client/src/services/platformManager.ts
  - client/src/contexts/ApplicationModeContext.tsx
  - client/src/services/SpaceDriverService.ts
  - client/src/features/visualisation/hooks/useSpacePilot.ts
  - client/src/features/visualisation/controls/SpacePilotController.ts
  - client/src/features/visualisation/components/SpacePilotSimpleIntegration.tsx
  - client/src/hooks/useHeadTracking.ts
  - client/src/features/visualisation/components/HeadTrackedParallaxController.tsx
  - client/src/features/graph/components/GraphManager.tsx
  - client/src/features/graph/components/InstancedLabels.tsx
  - client/vite.config.ts
  - client/package.json
  - xr-client/project.godot
  - client/src/app/App.tsx
  - client/src/services/remoteLogger.ts
verified_commit: 7a20db228
---

## VC-37.1 Browser XR capability probe — what platformManager actually does

```mermaid
sequenceDiagram
    autonumber
    participant AI as AppInitializer
    participant PM as usePlatformStore.initialize<br/>client/src/services/platformManager.ts:97
    participant DP as detectPlatform<br/>client/src/services/platformManager.ts:164
    participant NX as navigator.xr (WebXR Device API)
    participant W as window

    AI->>PM: initialize()
    PM->>DP: detectPlatform()
    DP->>DP: read navigator.userAgent
    alt UA contains "Quest 3"
        DP-->>DP: platform=quest3, xrDeviceType=quest
    else UA contains "Quest 2"
        DP-->>DP: platform=quest2, xrDeviceType=quest
    else UA contains "Quest"
        DP-->>DP: platform=quest, xrDeviceType=quest
    else UA contains "Pico" or "PICO"
        DP-->>DP: platform=pico, xrDeviceType=pico
    else Android/webOS/iPhone/iPad/iPod/BlackBerry/IEMobile/Opera Mini
        DP-->>DP: platform=mobile, xrDeviceType=mobile-xr
    else
        DP-->>DP: platform=desktop, xrDeviceType=desktop-xr
    end
    Note over DP: platformManager.ts:173-193. performanceTier low/medium/high<br/>is derived per branch at platformManager.ts:200-231
    alt navigator.xr present
        PM->>NX: isSessionSupported('immersive-vr')
        NX-->>PM: vrSupported
        PM->>NX: isSessionSupported('immersive-ar')
        NX-->>PM: arSupported
        PM->>PM: capabilities.xrSupported = vrSupported or arSupported
        Note over PM: platformManager.ts:105-122
    else navigator.xr absent
        PM-->>PM: capabilities untouched, isWebXRSupported stays false
    end
    opt navigator.xr present
        PM->>PM: capabilities.handTrackingSupported = isQuest()
        Note over PM: DIVERGENCE hand-tracking support is INFERRED from the<br/>user-agent, never queried. No WebXR feature descriptor is<br/>requested. platformManager.ts:129-141
    end
    PM->>W: addEventListener('resize', detectPlatform)
    PM->>PM: isWebXRSupported = !!navigator.xr - initialized = true
    Note over PM: platformManager.ts:152-158
    Note over AI,NX: RESOLVED ADR-2081: the probe never calls requestSession and now<br/>never claims to. @react-three/xr is removed from package.json and the<br/>XR-mode flags are deleted. What remains is exactly what is read:<br/>capability detection feeding remoteLogger telemetry - see VC-37.2.<br/>The immersive client is the Godot + OpenXR app - see VC-36.
```

## VC-37.2 The XR-mode surface is gone (RESOLVED ADR-2081)

```mermaid
flowchart TB
    subgraph kept["Retained — the capability probe (live, feeds telemetry)"]
        K1["isWebXRSupported = !!navigator.xr<br/>platformManager.ts:79, :140-144, getter :364"]
        K2["capabilities.xrSupported / vrSupported / arSupported<br/>from isSessionSupported platformManager.ts:95-97"]
        K3["capabilities.handTrackingSupported<br/>platformManager.ts:13, :119-124"]
        K4["xrDeviceType quest | pico | desktop-xr | mobile-xr | none<br/>platformManager.ts:34, :157"]
        K5["remoteLogger reports webXRSupported<br/>client/src/services/remoteLogger.ts:273, :281"]
    end
    subgraph gone["Deleted — modelled being IN a session, unreachable"]
        G1["XRSessionState type"]
        G2["isXRMode and xrSessionState store fields"]
        G3["setXRMode and setXRSessionState — had zero callers"]
        G4["xrmodechange and xrsessionstatechange events"]
        G5["ApplicationMode 'xr' member and its layout branch"]
        G6["isXRMode prop chain GraphManager to InstancedLabels"]
        G7["buildLabelLines vrMode parameter — was always false"]
        G8["@react-three/xr 6.6.29 dependency — imported nowhere"]
    end
    K1 --> K5
    K2 --> K5
    K3 --> K4
    gone --> R1
    R1["RESOLVED ADR-2081: the browser client has no XR session state.<br/>setXRMode/setXRSessionState had zero callers so isXRMode was<br/>permanently false. useApplicationMode had no consumers and<br/>setMode('xr') was never called, so the second flag was dead too.<br/>Removing the constant-false vrMode makes the five 'showMetadata<br/>&& !vrMode' conditionals unconditional — behaviour-preserving.<br/>The immersive client is the Godot + OpenXR app (ADR-2032) — see VC-36."]
    kept --> R2
    R2["The probe stays because it is READ: remoteLogger ships<br/>webXRSupported as telemetry. Capability detection is not<br/>session state and was never the dead part."]
```

## VC-37.3 ApplicationMode transitions and the mobile guard

```mermaid
stateDiagram-v2
    [*] --> desktop
    desktop --> mobile: resize and isMobile
    mobile --> desktop: resize and not isMobile
    note right of mobile
        RESOLVED ADR-2081 the 'xr' member is deleted
        ApplicationMode is now desktop | mobile only
        client/src/contexts/ApplicationModeContext.tsx:6
        setMode('xr') was never called and useApplicationMode
        had no consumers, so the xr state was unreachable
        The resize guard that read "mode !== 'xr'" is simplified
    end note
    note left of desktop
        useState default 'desktop' ApplicationModeContext.tsx:42
        isMobileView tracked separately ApplicationModeContext.tsx:44
        layoutSettings per mode, memoised on mode
        ApplicationModeContext.tsx:19, :27
        The provider stays mounted at client/src/app/App.tsx:157
    end note
```

## VC-37.4 SpaceMouse / SpacePilot — WebHID acquisition and secure-context gate

```mermaid
sequenceDiagram
    autonumber
    participant UI as SpacePilotSimpleIntegration.tsx<br/>client/src/features/visualisation/components/SpacePilotSimpleIntegration.tsx
    participant HK as useSpacePilot<br/>client/src/features/visualisation/hooks/useSpacePilot.ts:49
    participant SD as SpaceDriver singleton<br/>client/src/services/SpaceDriverService.ts:318
    participant IN as SpaceDriverService.initialize<br/>client/src/services/SpaceDriverService.ts:63
    participant HID as navigator.hid (WebHID)
    participant DV as HIDDevice

    UI->>HK: mount with enabled
    HK->>SD: initialize()
    SD->>IN: guard isInitialized
    IN->>HID: feature-detect navigator.hid
    alt navigator.hid absent
        IN-->>IN: warn 4 causes - not HTTPS/localhost, browser lacks WebHID,<br/>flag disabled, insecure context
        opt window.isSecureContext === false
            IN-->>IN: warn remediation - use http://localhost:3000, use HTTPS,<br/>or chrome://flags insecure-origins-treated-as-secure
            Note right of IN: SpaceDriverService.ts:76-84
        end
        IN->>UI: dispatchEvent('webhid-unavailable', {isSecureContext, hostname, protocol})
        Note right of IN: SpaceDriverService.ts:87-93 - BREAK, no device
    else navigator.hid present
        IN->>HID: getDevices()
        HID-->>IN: already-paired devices
        IN->>IN: filter on SUPPORTED_VENDOR_IDS
        Note over IN: VENDOR_ID_LOGITECH 0x046d SpaceDriverService.ts:11<br/>VENDOR_ID_3DCONNEXION 0x256f SpaceDriverService.ts:12<br/>SUPPORTED_VENDOR_IDS SpaceDriverService.ts:13
        alt at least one supported paired device
            loop each candidate until one opens
                IN->>DV: open()
                Note right of IN: Try every paired device - use whichever is<br/>physically connected. SpaceDriverService.ts:107-112
            end
            IN->>DV: addEventListener('inputreport', handleInputReport)
            Note right of IN: SpaceDriverService.ts:170
        else none paired
            IN-->>UI: idle until scan()
        end
    end
    UI->>SD: scan() on user gesture
    SD->>HID: requestDevice({filters: DEVICE_FILTERS})
    Note over SD,HID: DEVICE_FILTERS derived from the vendor ids<br/>SpaceDriverService.ts:14, REQUEST_PARAMS :15
    alt user picks nothing
        HID-->>SD: [] - return false
    else device chosen
        SD->>DV: openDevice(devices[0])
    end
    SD->>HID: listen for HID connect events
    opt evt.device.vendorId in SUPPORTED_VENDOR_IDS
        SD-->>UI: hot-plug adopt SpaceDriverService.ts:134
    end
```

## VC-37.5 SpacePilot HID report decode and camera drive

```mermaid
sequenceDiagram
    autonumber
    participant DV as HIDDevice inputreport
    participant HR as handleInputReport<br/>client/src/services/SpaceDriverService.ts:219
    participant HT as handleTranslation<br/>client/src/services/SpaceDriverService.ts:249
    participant HRo as handleRotation<br/>client/src/services/SpaceDriverService.ts:260
    participant HB as handleButtons<br/>client/src/services/SpaceDriverService.ts:271
    participant HK as useSpacePilot listeners<br/>client/src/features/visualisation/hooks/useSpacePilot.ts:127
    participant SC as SpacePilotController<br/>client/src/features/visualisation/controls/SpacePilotController.ts
    participant CAM as three PerspectiveCamera via useThree<br/>client/src/features/visualisation/hooks/useSpacePilot.ts:49

    DV-->>HR: inputreport event
    HR->>HR: values = new Int16Array(evt.data.buffer)
    Note over HR: log on reports 1-3 then every 1000th<br/>SpaceDriverService.ts:220-223
    alt reportId === REPORT_ID_TRANSLATION (1)
        alt values.length >= 6
            HR->>HT: handleTranslation(values)
            HR->>HRo: handleRotation(Int16Array[values3, values4, values5])
            Note right of HR: SpaceNavigator USB packs all 6 axes into report 1<br/>(12 bytes). SpaceDriverService.ts:228-231
        else 6 bytes
            HR->>HT: handleTranslation(values)
            Note right of HR: SpacePilot sends translation only in report 1<br/>SpaceDriverService.ts:233-234
        end
    else reportId === REPORT_ID_ROTATION (2)
        HR->>HRo: handleRotation(values)
    else reportId === REPORT_ID_BUTTONS (3)
        HR->>HB: handleButtons(new Uint16Array(buffer)[0])
    else unknown
        HR-->>HR: warn 'Unknown report ID' - BREAK
        Note right of HR: SpaceDriverService.ts:243-244
    end
    Note over HR: REPORT_ID_TRANSLATION=1, REPORT_ID_ROTATION=2,<br/>REPORT_ID_BUTTONS=3 SpaceDriverService.ts:18-20
    HT->>HK: CustomEvent 'translate' {x: v0, y: v1, z: v2}
    HRo->>HK: CustomEvent 'rotate' {rx: -v0, ry: -v1, rz: v2}
    Note over HRo: INVARIANT rx and ry are NEGATED, rz is not -<br/>SpaceDriverService.ts:262-266
    HB->>HB: walk 16 bits of buttonBits
    HB->>HK: CustomEvent 'buttons' {buttons: ['[1]','[A]', ...]}
    Note over HB: label = (i+1).toString(16).toUpperCase() so button 10<br/>reads [A]. SpaceDriverService.ts:273-284
    HK->>SC: forward to controller with camera and orbitControlsRef
    Note over HK,SC: listeners registered for translate, rotate, buttons,<br/>connect, disconnect useSpacePilot.ts:127-131 and torn<br/>down symmetrically at :138-142
    SC->>CAM: apply per mode
    Note over SC,CAM: currentMode camera | object | navigation<br/>useSpacePilot.ts:27-28, :52. Effect re-binds on<br/>enabled, camera, orbitControlsRef, isSupported,<br/>spacePilotSettings, userConfig useSpacePilot.ts:96
```

## VC-37.6 Head-tracked parallax — MediaPipe face landmarks to camera frustum

```mermaid
sequenceDiagram
    autonumber
    participant C as HeadTrackedParallaxController<br/>client/src/features/visualisation/components/HeadTrackedParallaxController.tsx:14
    participant H as useHeadTracking<br/>client/src/hooks/useHeadTracking.ts:13
    participant FR as FilesetResolver.forVisionTasks<br/>client/src/hooks/useHeadTracking.ts:27
    participant FL as FaceLandmarker<br/>@mediapipe/tasks-vision
    participant V as HTMLVideoElement webcam
    participant CAM as three PerspectiveCamera via useThree<br/>HeadTrackedParallaxController.tsx:14

    C->>H: useHeadTracking()
    H->>H: initialize() - skip if faceLandmarker already set
    H->>FR: forVisionTasks('https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@0.10.21/wasm')
    H->>FL: createFromOptions({modelAssetPath '/models/face_landmarker.task', delegate 'GPU',<br/>runningMode 'VIDEO', numFaces 1, no blendshapes, no transform matrices})
    Note over H,FL: useHeadTracking.ts:29-40
    alt init throws
        H-->>C: error 'Failed to load head tracking model. Please check your network connection.'
        Note right of H: useHeadTracking.ts:41-44 - isTracking stays false
    end
    loop requestAnimationFrame predictWebcam
        H->>V: read currentTime
        alt no video, no landmarker, or no srcObject
            H-->>H: cancelAnimationFrame - BREAK
            Note right of H: useHeadTracking.ts:47-50
        end
        opt currentTime !== lastVideoTime
            H->>FL: detectForVideo(video, Date.now())
            FL-->>H: faceLandmarks
            opt at least one face
                H->>H: take nose tip landmark index 1
                H->>H: lerp into smoothedPosition
                Note right of H: SMOOTHING_FACTOR = 0.15 useHeadTracking.ts:11<br/>smoothedPosition is a THREE.Vector2 ref useHeadTracking.ts:21
            end
        end
    end
    H-->>C: {isEnabled, setIsEnabled, isTracking, headPosition, error}
    C->>C: cameraMode from settings.visualisation.interaction.headTrackedParallax.cameraMode<br/>default 'asymmetricFrustum' HeadTrackedParallaxController.tsx:19
    loop useFrame
        alt isTracking and headPosition and camera is PerspectiveCamera
            alt cameraMode === 'asymmetricFrustum'
                C->>CAM: setViewOffset(...) then updateProjectionMatrix()
                Note right of C: HeadTrackedParallaxController.tsx:46-54
            else
                C->>CAM: projectionMatrix.multiply(NUDGE_MATRIX.makeTranslation(offsetX, offsetY, 0))
                Note right of C: HeadTrackedParallaxController.tsx:57-62 - the module-level<br/>NUDGE_MATRIX replaced a per-frame Matrix4 plus Vector3 allocation (:60-61)
            end
        else not tracking
            opt camera.view set
                C->>CAM: clearViewOffset() then updateProjectionMatrix()
                Note right of C: restores the plain frustum<br/>HeadTrackedParallaxController.tsx:66-68
            end
        end
    end
    Note over C,CAM: This is the desktop stand-in for stereo parallax. It shares the<br/>R3F camera with the graph render path - see VC-31 - and touches<br/>no SharedArrayBuffer of its own.
```

## VC-37.7 How the desktop XR-adjacent inputs share state with the R3F path

```mermaid
flowchart LR
    subgraph inputs["Desktop spatial inputs"]
        SP["SpaceDriver WebHID singleton<br/>client/src/services/SpaceDriverService.ts:318"]
        HT["useHeadTracking MediaPipe<br/>client/src/hooks/useHeadTracking.ts:13"]
    end
    subgraph r3f["Shared R3F context (useThree)"]
        CAM["PerspectiveCamera"]
        SCN["scene"]
        GL["gl renderer"]
    end
    subgraph graph["Graph render path (VC-31)"]
        GMx["GraphManager.tsx useFrame<br/>client/src/features/graph/components/GraphManager.tsx"]
        SAB["SharedArrayBuffer node positions"]
        ILx["InstancedLabels.tsx<br/>client/src/features/graph/components/InstancedLabels.tsx"]
    end
    SP --> HKx["useSpacePilot useThree camera, scene, gl<br/>useSpacePilot.ts:49"] --> CAM
    HT --> HPCx["HeadTrackedParallaxController useThree camera, size<br/>HeadTrackedParallaxController.tsx:14"] --> CAM
    CAM --> GMx
    SCN --> GMx
    GL --> GMx
    GMx --> SAB
    SAB --> ILx
    GMx --> ILx
    ILx --> NOTE1
    NOTE1["RESOLVED ADR-2081: the isXRMode prop chain from the platform store<br/>through GraphManager into InstancedLabels is deleted, along with the<br/>constant-false vrMode parameter of buildLabelLines. Label layout no<br/>longer takes an XR input, because there was never an XR session to<br/>signal. Metadata lines previously gated on !vrMode now render<br/>whenever showMetadata is set - behaviour-preserving."]
    inputs --> NOTE2
    NOTE2["INVARIANT both spatial inputs mutate the SAME R3F camera and<br/>write nothing into the SharedArrayBuffer. Position data flows<br/>one way - websocket to SAB to renderer (VC-32, VC-31) - so an<br/>input device can never desynchronise graph state."]
```

## VC-37.8 Browser path versus the shipped Godot path

```mermaid
flowchart TB
    subgraph br["Browser client (client/)"]
        B1["Three.js / R3F, WebGL2 or WebGPU<br/>see VC-31"]
        B2["navigator.xr capability probe only<br/>platformManager.ts:105-122"]
        B3["No requestSession, no XRButton, no XR session state<br/>(RESOLVED ADR-2081 - the shell is deleted)"]
        B4["Spatial input = WebHID SpaceMouse<br/>+ MediaPipe head parallax"]
        B5["Binary V3/V5 over /wss to SAB<br/>see VC-32"]
    end
    subgraph gd["Godot XR client (xr-client/)"]
        G1["Godot 4 + gdext, OpenXR<br/>forced gl_compatibility project.godot:48"]
        G2["Real stereo submission on SteamVR<br/>ADR-2032"]
        G3["OpenXR action map, 14 interaction profiles<br/>xr-client/openxr_action_map.tres"]
        G4["Hand ray + pinch, head/eye gaze<br/>see VC-36"]
        G5["Same binary V3/V5 wire, decoded in Rust<br/>see VC-36.5"]
    end
    B5 --- SHARED["Shared server surface: /wss binary position stream,<br/>/api REST. Identical V3 52-byte record and V5 envelope."]
    G5 --- SHARED
    B2 --> DIV
    G2 --> DIV
    DIV["RESOLVED ADR-2081: docs/BASELINE-architecture.md:195-196 describes the<br/>React client as 'consuming the binary WebSocket position stream and the<br/>/api REST surface' - which is now exactly what the code is. The XR-mode<br/>shell and the unimported @react-three/xr dependency are deleted, so no<br/>reader can mistake the browser client for an immersive one. The immersive<br/>client is the Godot app alone. Babylon.js was already removed<br/>(client/vite.config.ts:47)."]
```
