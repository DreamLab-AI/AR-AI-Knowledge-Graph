---
id: VC-31
title: R3F/Three.js graph render pipeline and WASM scene effects
area: visionclaw
verified_commit: bed6b617d
governing:
  - docs/BASELINE-architecture.md
adrs: []
sources:
  - client/src/app/MainLayout.tsx
  - client/src/features/graph/components/GraphCanvas.tsx
  - client/src/features/graph/components/GraphCanvasWrapper.tsx
  - client/src/features/graph/components/GraphManager.tsx
  - client/src/features/graph/components/GemNodes.tsx
  - client/src/features/graph/components/GlassEdges.tsx
  - client/src/features/graph/components/InstancedLabels.tsx
  - client/src/features/graph/components/KnowledgeRings.tsx
  - client/src/features/graph/components/ClusterHulls.tsx
  - client/src/features/graph/components/InferredEdges.tsx
  - client/src/features/graph/components/TimelineScrubber.tsx
  - client/src/features/graph/components/PerfProbe.tsx
  - client/src/features/graph/components/NodeContextMenu.tsx
  - client/src/features/graph/components/NodeDetailPanel.tsx
  - client/src/features/graph/hooks/useEdgeBufferComputation.ts
  - client/src/features/graph/utils/nodeScaling.ts
  - client/src/features/graph/hooks/useFpsMonitor.ts
  - client/src/features/graph/managers/graphWorkerProxy.ts
  - client/src/features/graph/hooks/useGraphSelection.ts
  - client/src/store/transientBeamStore.ts
  - client/src/store/websocket/binaryProtocol.ts
  - client/src/rendering/GemPostProcessing.tsx
  - client/src/rendering/postProcessingGuard.ts
  - client/src/rendering/rendererFactory.ts
  - client/src/rendering/troikaConfig.ts
  - client/src/rendering/materials/GemNodeMaterial.ts
  - client/src/rendering/materials/CrystalOrbMaterial.ts
  - client/src/rendering/materials/AgentCapsuleMaterial.ts
  - client/src/rendering/materials/GlassEdgeMaterial.ts
  - client/src/rendering/text/createTextMaterial.ts
  - client/src/rendering/text/GlyphAtlas.ts
  - client/src/rendering/text/textLayout.ts
  - client/src/wasm/scene-effects-bridge.ts
  - client/src/hooks/useWasmSceneEffects.ts
  - client/src/features/visualisation/components/WasmSceneEffects.tsx
  - client/src/features/visualisation/components/TransientBeamsLayer.tsx
  - client/src/features/visualisation/components/EmbeddingCloudLayer.tsx
  - client/src/features/visualisation/hooks/useTransientBeams.ts
  - client/src/features/visualisation/attentionHeat.ts
  - client/src/features/visualisation/heatColor.ts
  - client/src/features/visualisation/semanticEncoding.ts
  - client/src/features/visualisation/cameraFocus.ts
  - client/crates/scene-effects/src/lib.rs
  - client/crates/scene-effects/src/particles.rs
  - client/crates/scene-effects/src/energy_wisps.rs
  - client/crates/scene-effects/src/atmosphere.rs
  - client/src/wasm/scene-effects/scene_effects.js
---
## VC-31.1 Canvas consumers and shared child tree
```mermaid
flowchart LR
    ML["MainLayoutContent<br/>client/src/app/MainLayout.tsx:18"] -->|mounts| GCW["GraphCanvasWrapper<br/>components/GraphCanvasWrapper.tsx:134"]
    GCW -->|CanvasErrorBoundary wraps| GC["GraphCanvas<br/>components/GraphCanvas.tsx:275"]
    GC -->|"gl=createGemRenderer"| CANVAS["R3F Canvas<br/>components/GraphCanvas.tsx:361"]
    CANVAS --> CAS["CameraAspectSync<br/>GraphCanvas.tsx:159"]
    CANVAS --> EEC["EnsureEventsConnected<br/>GraphCanvas.tsx:203"]
    CANVAS --> PP["PerfProbe (DEV only, lazy)<br/>components/PerfProbe.tsx:385"]
    CANVAS --> ENV["Environment plus Lightformer, auto policy skips on software renderer"]
    CANVAS --> WSE["WasmSceneEffects<br/>visualisation/WasmSceneEffects.tsx:628"]
    CANVAS --> ECL["EmbeddingCloudLayer<br/>visualisation/EmbeddingCloudLayer.tsx:92"]
    CANVAS -->|"canvasReady and nodeCount above 0"| GM["GraphManager<br/>components/GraphManager.tsx:41"]
    GM --> GN["GemNodes x3 populations<br/>components/GemNodes.tsx:150"]
    GM --> GE["GlassEdges x2 (main + highlight)<br/>components/GlassEdges.tsx:180"]
    GM --> IE["InferredEdges<br/>components/InferredEdges.tsx:36"]
    GM --> KR["KnowledgeRings<br/>components/KnowledgeRings.tsx:32"]
    GM --> CH["ClusterHulls<br/>components/ClusterHulls.tsx:223"]
    GM --> TBL["TransientBeamsLayer<br/>visualisation/TransientBeamsLayer.tsx:69"]
    GM --> IL["InstancedLabels<br/>components/InstancedLabels.tsx:294"]
    CANVAS --> OC["OrbitControls (makeDefault)"]
    CANVAS --> GPP["GemPostProcessing<br/>client/src/rendering/GemPostProcessing.tsx"]
    GC --> LMI["LayoutModeIndicator (HTML overlay)"]
    GC -->|"provenance.enableTimeline"| TS["TimelineScrubber<br/>components/TimelineScrubber.tsx:75"]
    ML --> NDP["NodeDetailPanel (HTML overlay)<br/>components/NodeDetailPanel.tsx:56"]
    ML --> NCM["NodeContextMenu (HTML overlay)<br/>components/NodeContextMenu.tsx:41"]
    GM -.->|"visionclaw:node-contextmenu CustomEvent"| NCM
    Note1["Note: no GraphViewport.tsx exists in this repo -- retired, per<br/>GraphCanvas.tsx:467 comment. NodeDetailPanel/NodeContextMenu are siblings of<br/>GraphCanvasWrapper in MainLayout, not children of GraphManager."]
```

## VC-31.2 GraphManager per-frame hot loop (priority -2)
```mermaid
sequenceDiagram
    autonumber
    participant R3F as R3F render loop
    participant GM as GraphManager.useFrame priority -2<br/>GraphManager.tsx:360
    participant WP as graphWorkerProxy.getPositionsSync<br/>graphWorkerProxy.ts:294
    participant SEL as useGraphSelection fly-to state<br/>useGraphSelection.ts:67, resolveNodeWorldPosition cameraFocus.ts:68
    participant EBC as useEdgeBufferComputation.useFrame priority -2<br/>useEdgeBufferComputation.ts:73
    participant NS as computeNodeScale<br/>nodeScaling.ts:36
    participant GE as GlassEdgesHandle<br/>GlassEdges.tsx:454-542

    rect rgb(232,238,250)
    Note over GM: INVARIANT both useFrame hooks register at priority -2, order among same-priority callbacks is registration order (React effect order)
    R3F->>GM: frame(state, delta)
    opt flyToTargetRef.current set
        GM->>SEL: read flyToTargetRef, flyToLookAtRef, flyToProgressRef
        GM->>GM: flyToProgressRef += delta / 0.6 (~600ms envelope, eased 1-(1-p)^3)
        alt p >= 1
            GM->>GM: camera.position.copy(dest), cancelFlyTo()
        else in flight
            GM->>GM: camera.position.lerpVectors(start, dest, eased)
        end
    end
    GM->>GM: labelTickRef++ (every frame)
    alt labelTickRef >= 15 (~4 updates/sec at 60fps)
        GM->>GM: frustum.setFromProjectionMatrix, setLabelUpdateTick(prev+1)
    end
    GM->>WP: getPositionsSync()
    alt positions null
        GM-->>R3F: return (skip rest of frame)
    else positions available
        WP-->>GM: Float32Array SAB view
        opt first frame and all-zero prefix
            GM-->>R3F: return (wait for non-zero positions)
        end
        GM->>GM: nodePositionsRef.current = positions
        opt transitionRef.current.active (layout mode change)
            GM->>GM: eased progress = easeInOutQuad(rawProgress)
            loop for each node i
                GM->>GM: massFactor = 1 / (1 + sqrt(connectionCount)*0.3)
                GM->>GM: positions[i] = lerp(start, target, min(progress/massFactor,1))
            end
            alt rawProgress >= 1.0
                GM->>GM: transitionRef.current.active = false, setLayoutTransitioning(false)
            end
        end
        GM->>GM: requestCameraFit()
        alt positions.length >= nodes.length*3
            loop for each node i (labelPositionsRef rebuild, every frame)
                GM->>GM: labelArr[i] = positions[i3..i3+2]
            end
        end
    end
    end

    rect rgb(240,232,250)
    Note over EBC: separate useFrame(-2) hook, extracted from GraphManager (Phase B1)
    R3F->>EBC: frame()
    alt positions null or positions.length < nodes.length*3
        EBC-->>R3F: return
    else sufficient
        loop for each edge in graphData.edges
            alt source or target not in visibleNodeIds (renderedNodeIds)
                EBC->>EBC: skip edge (pruned endpoint)
            else both endpoints rendered
                EBC->>NS: computeNodeScale(sourceNode) * nodeSize
                NS-->>EBC: srcR
                EBC->>NS: computeNodeScale(targetNode) * nodeSize
                NS-->>EBC: tgtR
                EBC->>EBC: srcOff = srcPos + dir*srcR, tgtOff = tgtPos - dir*tgtR (surface-to-surface)
                alt distance(srcOff,tgtOff) > 0.1
                    EBC->>EBC: write 6 floats to edgeBuffer, RGB to edgeColors, weight to edgeWeights
                end
            end
        end
        opt selectedNodeId set
            EBC->>EBC: rebuild highlightBuffer for edges touching selectedNodeId (threshold 0.2)
            EBC->>GE: highlightEdgeFlowRef.updatePoints(hlBuf, hlIdx)
        end
        EBC->>GE: edgeFlowRef.updateWidths(edgeWeights, count) — BEFORE updatePoints
        EBC->>GE: edgeFlowRef.updatePoints(newEdgePoints, edgePointIdx)
        opt edgeColorIdx > 0
            EBC->>GE: edgeFlowRef.updateColors(edgeColors, edgeCountWithColor)
        end
    end
    end
    Note over GM,EBC: see VC-30 for graphDataManager and graph worker SAB plumbing upstream of getPositionsSync
```

## VC-31.3 InstancedLabels two-phase useFrame (WebGL path)
```mermaid
sequenceDiagram
    autonumber
    participant PARENT as InstancedLabels wrapper<br/>InstancedLabels.tsx:294
    participant WGL as InstancedLabelsWebGL.useFrame priority 0<br/>InstancedLabels.tsx:591
    participant GEO as InstancedBufferGeometry aLabelPos etc<br/>InstancedLabels.tsx:556-575
    participant LAYOUT as layoutTextInline<br/>textLayout.ts
    participant ATLAS as createGlyphAtlas<br/>GlyphAtlas.ts

    Note over PARENT: isWebGPURenderer routes to InstancedLabelsWebGPU (Html overlay, InstancedLabels.tsx:328) instead of this WebGL instanced path
    Note over PARENT: INVARIANT nodePositionsRef must be forwarded by every caller. Historical bug: a missing forward silently fell back to stale labelPositionsRef, logged once at InstancedLabels.tsx:636-643
    WGL->>WGL: frameCountRef++ , camera motion check (posDelta>0.5 or rotDelta>0.001)
    alt cameraMovingFast
        WGL->>GEO: geometry.instanceCount = 0 (hide all labels)
        WGL-->>PARENT: return
    else within 150ms debounce of last fast motion
        WGL-->>PARENT: return
    else camera settled
        rect rgb(232,248,238)
        Note over WGL,GEO: Phase 1 -- every still frame: patch aLabelPos from SAB (InstancedLabels.tsx:627-678)
        loop for each entry in nodeGlyphMapRef (existing glyphs)
            WGL->>WGL: wx,wy,wz = rawPositions[physicsIndex*3 .. +2] plus labelOffsetY on Y
            loop for g in glyphStart..glyphStart+glyphCount
                WGL->>GEO: labelPosArr[g] = wx,wy,wz
            end
        end
        WGL->>GEO: labelPosAttr.needsUpdate = true
        end
        alt frameCountRef % labelLayoutEvery !== 0 (default labelLayoutEvery=3, settings.rendering.labelLayoutEvery) and nodeGlyphMapRef non-empty
            WGL-->>PARENT: return (skip phase 2 this frame)
        else every Nth frame -- full layout rebuild
            rect rgb(250,232,235)
            Note over WGL,LAYOUT: Phase 2 -- frustum cull + layoutTextInline (InstancedLabels.tsx:711-884)
            WGL->>WGL: widen frustum by 0.9x scale on projection elements 0,5 (~10% margin)
            WGL->>WGL: sort nodes by squared distance to camera (closest first)
            loop for each node in distance order
                alt not in widened frustum, or distance out of [2, LABEL_DISTANCE_THRESHOLD]
                    WGL->>WGL: skip node
                else candidate
                    WGL->>WGL: project to NDC, map to 32x18 declutter grid cell
                    alt cell already occupied by a closer label
                        WGL->>WGL: cellsRejected++ , skip
                    else cell free
                        WGL->>WGL: computeNodeScale (nodeScaling.ts:36) for labelOffsetY
                        WGL->>LAYOUT: layoutTextInline(lines, atlas, maxWidth, buffers.., glyphIdx, MAX_GLYPHS=32768)
                        LAYOUT-->>WGL: glyphCount written directly into aLocalOffset/aScale/aUVRect/aColor/aOpacity/aLabelPos
                        WGL->>WGL: newNodeMap.push({nodeId, physicsIndex, glyphStart, glyphCount, labelOffsetY})
                    end
                end
            end
            WGL->>GEO: geometry.instanceCount = glyphIdx, mark all 6 attribute buffers needsUpdate
            end
        end
    end
    Note over PARENT,ATLAS: createGlyphAtlas and createTextMaterial run once in useMemo (InstancedLabels.tsx:542-580), not per frame
```

## VC-31.4 GlassEdges instancing -- allocation, matrix composition, imperative hot path
```mermaid
sequenceDiagram
    autonumber
    participant CALLER as useEdgeBufferComputation<br/>useEdgeBufferComputation.ts:73
    participant GE as GlassEdges<br/>GlassEdges.tsx:180
    participant GEOM as createGlassEdgeGeometry<br/>GlassEdgeMaterial.ts:195
    participant CIM as computeInstanceMatrices<br/>GlassEdges.tsx:104
    participant MESH as THREE.InstancedMesh

    Note over GEOM: CylinderGeometry(radius, radius, height=1, radialSegments=4,<br/>heightSegments=1, openEnded=true). Default radius 0.03, overridden by settings.baseWidth<br/>(edgeRadius, GlassEdges.tsx:209)
    Note over GE: EDGE_INITIAL=1024 capacity, DEFAULT_EDGE_CEILING=65536, grows x2 on<br/>overflow up to settings.visualisation.rendering.maxEdgesCeiling (ADR-04 D1/D3/D4)

    rect rgb(234,238,250)
    Note over GE,MESH: Mount -- initial allocation (GlassEdges.tsx:280-319)
    GE->>GEOM: allocateMesh(EDGE_INITIAL, edgeRadius, colorOverride, opacity)
    GEOM-->>GE: InstancedMesh(capacity=1024) plus white instanceColor buffer
    opt points.length >= 6 at mount
        GE->>GE: sized = min(ceilToPowerOfTwo(initialEdgeCount*1.25), ceiling)
        opt sized > EDGE_INITIAL
            GE->>GEOM: reallocate to sized capacity before reveal starts
        end
        GE->>CIM: computeInstanceMatrices(mesh, points, capacity, limit=EDGE_REVEAL_BATCH)
    end
    end

    rect rgb(236,250,232)
    Note over CALLER,MESH: Imperative hot path -- CALLER runs every frame at useFrame priority -2
    CALLER->>GE: updateWidths(weights, count)
    GE->>GE: cheap sig = hash(count, sparse weight samples)
    alt sig unchanged and radiusFactorsRef sized enough
        GE->>GE: skip recompute (weights static between data changes)
    else weights changed
        loop for i in 0..count
            GE->>GE: factors[i] = weightToRadiusFactor(weight) = clamp(sqrt(weight/1.0), 0.5, 2.0)
        end
    end
    CALLER->>GE: updatePoints(newPts, count)
    GE->>GE: imperativeActiveRef = true (prop-driven progressive reveal now disabled permanently)
    GE->>GE: ensureCapacity(edgeCount) -- reallocate x2 up to ceiling, else clamp and<br/>console.info/warn
    GE->>CIM: computeInstanceMatrices(mesh, newPts, capacity, undefined, len, radiusFactors)
    loop for i in 0..renderCount
        CIM->>CIM: midpoint = (src+tgt)*0.5
        CIM->>CIM: dir = normalize(tgt-src), len = |tgt-src|
        alt len < 1e-6 (collapsed/overlapping nodes)
            CIM->>MESH: setMatrixAt(i, scale(0,0,0))
        else
            CIM->>CIM: quat = setFromUnitVectors(up=(0,1,0), dir), guard dot < -0.9999 anti-parallel
            CIM->>CIM: scale = (radiusFactor, len, radiusFactor) -- Y scale IS the edge length
            CIM->>MESH: setMatrixAt(i, compose(midpoint, quat, scale))
        end
    end
    CIM->>MESH: mesh.count = renderCount, instanceMatrix.needsUpdate = true
    opt edgeColorIdx > 0
        CALLER->>GE: updateColors(colors, count)
        GE->>MESH: instanceColor.array.set(colors), instanceColorsActiveRef=true, mat.color=white
    end
    end

    rect rgb(252,234,236)
    Note over GE: DIVERGENCE (historical, fixed): prior updatePoints dedup used a hash of<br/>only 3 values (len, points[0], points[len-1]) -- GraphManager.tsx comment and this<br/>file's comment (GlassEdges.tsx:451-453) confirm this froze edges when those 3 sampled<br/>values were stable but interior points moved. Fix: always recompute matrices, no hash<br/>gate.
    Note over GE: own useFrame (no priority arg, default 0): emissive pulse only (sin wave),<br/>plus progressive reveal ramp while NOT imperativeActiveRef (GlassEdges.tsx:547-568)
    end
```

## VC-31.5 Node geometries and materials per population
```mermaid
classDiagram
    class GemNodes {
        GemNodes.tsx:150 GemNodesInner
        dominant knowledge_graph ontology or agent decides geometry plus material at mesh creation
        capacity = nextPowerOf2 max nodes.length 4096
    }
    class GemGeometry {
        GemNodeMaterial.ts:278 createGemGeometry
        THREE.IcosahedronGeometry radius 0.5 detail 1
        population knowledge
    }
    class CrystalOrbGeometry {
        CrystalOrbMaterial.ts:165 createCrystalOrbGeometry
        THREE.IcosahedronGeometry radius 0.5 detail 1
        population ontology, faceted 80 tris not smooth sphere
    }
    class AgentCapsuleGeometry {
        AgentCapsuleMaterial.ts:177 createAgentCapsuleGeometry
        THREE.CapsuleGeometry radius 0.3 length 0.6 capSegments 4 radialSegments 8
        population agent
    }
    class GemNodeMaterial {
        GemNodeMaterial.ts:39 createGemNodeMaterial
        MeshStandardMaterial roughness 0.08 transparent true
        WebGL opacity 0.7 metalness 0 WebGPU opacity 0.85 metalness 0.15
        TSL emissiveNode plus opacityNode augmentation on WebGPU only
    }
    class CrystalOrbMaterial {
        CrystalOrbMaterial.ts:36 createCrystalOrbMaterial
        TSL metadata upgrade path shared pattern with GemNodeMaterial
    }
    class AgentCapsuleMaterial {
        AgentCapsuleMaterial.ts:36 createAgentCapsuleMaterial
        createTslAgentCapsuleMaterial WebGPU TSL variant, GemNodes.tsx:261
    }
    class InstancedMesh {
        THREE.InstancedMesh count instances
        setColorAt per-instance grey init 0.5 0.5 0.5
        setMatrixAt per-instance transform
    }
    GemNodes ..> GemGeometry : builds when dominant knowledge
    GemNodes ..> CrystalOrbGeometry : builds when dominant ontology
    GemNodes ..> AgentCapsuleGeometry : builds when dominant agent
    GemGeometry --> GemNodeMaterial : paired
    CrystalOrbGeometry --> CrystalOrbMaterial : paired
    AgentCapsuleGeometry --> AgentCapsuleMaterial : paired
    GemNodeMaterial --> InstancedMesh : material of
    CrystalOrbMaterial --> InstancedMesh : material of
    AgentCapsuleMaterial --> InstancedMesh : material of
    class AttentionHeatAccumulator {
        attentionHeat.ts:119 createAttentionHeatAccumulator
        touch(nodeId) on 0x23 AGENT_ACTION target, getHeat(nodeId) decayed 0..1 via normaliseHeat
        DEFAULT_HEAT_HALF_LIFE_MS 20000ms, HEAT_PER_TOUCH 1, MAX_RAW_HEAT 6
        id masking shared between touch and getHeat -- BOTH use the masked node id
    }
    class HeatColor {
        heatColor.ts:26 heatGain, heatColor.ts:36 heatBrightenFactor
        HEAT_BRIGHTEN_K 0.8 -- brightens base rgb toward white proportional to heat
    }
    GemNodes ..> AttentionHeatAccumulator : reads getHeat per instance
    GemNodes ..> HeatColor : heatBrightenFactor before setColorAt
```

## VC-31.6 Renderer selection and post-processing chain
```mermaid
sequenceDiagram
    autonumber
    participant CANVAS as R3F Canvas gl prop
    participant RF as createGemRenderer<br/>rendererFactory.ts:106
    participant NAV as navigator.gpu
    participant WGPU as WebGPURenderer three/webgpu
    participant GPP as GemPostProcessing<br/>GemPostProcessing.tsx:66
    participant GUARD as postProcessingGuard<br/>postProcessingGuard.ts:44

    CANVAS->>RF: await createGemRenderer(defaultProps)
    alt forceWebGLOverride (localStorage visionclaw-force-webgl)
        RF->>RF: skip WebGPU gate entirely
    else not forced
        RF->>NAV: check navigator.gpu present
        alt navigator.gpu absent
            RF->>RF: log navigator.gpu not available, go straight to WebGL
        else navigator.gpu present
            RF->>WGPU: new WebGPURenderer(forceWebGL:false), race init() vs 5s timeout
            alt init throws or times out
                RF->>RF: catch, log warn, fall through to WebGL fallback
            else backendName === WebGLBackend (silent internal fallback, r182 and earlier behaviour retained as a guard)
                RF->>WGPU: renderer.dispose()
                RF->>RF: throw, fall through to WebGL fallback
            else true WebGPU backend confirmed
                RF->>WGPU: toneMapping=ACESFilmic, outputColorSpace=SRGB, setSize from canvas rect
                RF->>RF: isWebGPURenderer=true, rendererCapabilities.backend=webgpu, tslMaterialsActive=true
                RF-->>CANVAS: return WebGPURenderer instance
            end
        end
    end
    opt WebGL fallback path
        RF->>RF: new THREE.WebGLRenderer(antialias, alpha, high-performance)
        RF->>RF: isWebGPURenderer=false, rendererCapabilities.backend=webgl
        RF-->>CANVAS: return WebGLRenderer instance
    end

    Note over GPP: mounted as CANVAS child regardless of backend, reads gl.__isWebGPURenderer to branch
    alt isWebGPURenderer true and glow or bloom setting enabled
        GPP->>GPP: dynamic import three/webgpu RenderPipeline, three/tsl pass, BloomNode.js bloom
        GPP->>GPP: build node graph scenePass to toTexture to bloom to add, outputNode
        GPP->>GPP: new RenderPipeline(gl, outputNode), setPipelineReady(true)
        Note over GPP: priority 1 useFrame -- R3F skips its own gl.render() call whenever any subscriber priority above 0, so this becomes the sole renderer
    else WebGL path and glow or bloom enabled
        GPP->>GPP: dynamic import EffectComposer, RenderPass, UnrealBloomPass
        GPP->>GPP: render target capped to maxDim 2048 on long edge (GPU memory guard)
        GPP->>GPP: composer.addPass(RenderPass), composer.addPass(UnrealBloomPass(strength,radius,threshold))
        GPP->>GPP: setPipelineReady(true), priority 1 useFrame takes over rendering
    else post-processing disabled or not yet ready
        Note over GPP: pipelineReady stays false -- R3F's default renderer draws the scene directly, GPP issues no render call
    end

    loop every frame while renderActive (pipelineReady and not ppDisabled)
        GPP->>GPP: try postProcessingRef.render() or composerRef.render()
        alt render succeeds
            GPP->>GUARD: recordRenderSuccess(state) -- resets consecutiveFailures to 0
        else render throws (GPU context loss/restore transient)
            GPP->>GUARD: recordRenderFailure(state, maxConsecutiveFailures=8)
            alt first failure of streak
                GPP->>GPP: logger.warn once per streak (not per frame)
            end
            alt consecutiveFailures >= 8
                GPP->>GPP: dispose pipeline, setPipelineReady(false), setPpDisabled(true)
                Note over GPP: INVARIANT terminal state -- never retries again this session, hands every future frame back to R3F default renderer
            end
        end
    end
```

## VC-31.7 Text rendering -- glyph atlas, billboard material, troika worker disable
```mermaid
sequenceDiagram
    autonumber
    participant BOOT as app boot (imports troikaConfig)<br/>troikaConfig.ts:20
    participant IL as InstancedLabelsWebGL useMemo<br/>InstancedLabels.tsx:542
    participant ATLAS as createGlyphAtlas<br/>GlyphAtlas.ts:32
    participant MAT as createTextMaterial<br/>createTextMaterial.ts:65
    participant LAY as layoutTextInline<br/>textLayout.ts:87

    Note over BOOT: DIVERGENCE from brief assumption -- this atlas is a plain<br/>canvas-rasterised bitmap glyph sheet (black outline plus white fill), NOT a<br/>signed-distance-field atlas. No SDF technique exists in this codebase's InstancedLabels<br/>path.
    BOOT->>BOOT: configureTextBuilder({useWorker:false}) IF self.crossOriginIsolated
    Note over BOOT: page is served with COOP same-origin plus COEP require-corp for the<br/>SharedArrayBuffer physics pipeline. Chromium blocks importScripts of blob colon URLs<br/>inside blob-created workers under require-corp (crbug 1084951), which is exactly how<br/>troika-worker-utils boots its glyph-layout worker. Remedy: build troika text on the main<br/>thread, only used for small per-agent labels, NOT the bulk InstancedLabels path below.

    IL->>ATLAS: createGlyphAtlas(fontSize=48) (module-level cache, first call wins)
    ATLAS->>ATLAS: create 1024x1024 canvas, ctx.font=48px system-ui sans-serif
    loop for each char in ATLAS_CHARS (ASCII plus a small symbol set for stars, hearts, arrows)
        ATLAS->>ATLAS: measure, wrap row if cursorX+charWidth exceeds 1024
        ATLAS->>ATLAS: strokeText black outline lineWidth 3, then fillText white fill
        ATLAS->>ATLAS: metrics.set(char, {u,v,w,h,advance,xOffset,yOffset}) normalized 0..1
    end
    ATLAS-->>IL: CanvasTexture (flipY false, LinearFilter, no mipmaps) plus metrics Map plus<br/>lineHeight
    IL->>MAT: createTextMaterial(atlas.texture) (module-level cache, first call wins)
    MAT->>MAT: ShaderMaterial uAtlas/uCamRight/uCamUp uniforms, transparent, depthWrite<br/>false, DoubleSide
    Note over MAT: vertex shader billboards each glyph quad via uCamRight/uCamUp camera<br/>basis vectors, worldPos = aLabelPos + localOffset. Fragment shader samples texSample.r<br/>as both alpha and colour multiplier, discards below 0.01 alpha.

    rect rgb(242,248,230)
    Note over IL,LAY: every labelLayoutEvery-th frame (see VC-31.3 phase 2)
    IL->>LAY: layoutTextInline(lines, atlas, maxWidth, aLocalOffset, aScale, aUVRect,<br/>aColor, aOpacity, aLabelPos, px,py,pz, opacity, glyphIdx, MAX_GLYPHS)
    LAY-->>IL: glyphCount, buffers written in place (zero allocation)
    end
```

## VC-31.8 WASM scene-effects zero-copy -- init, tick, view rebuild, dispose
```mermaid
sequenceDiagram
    autonumber
    participant COMP as WasmSceneEffects<br/>WasmSceneEffects.tsx:628
    participant HOOK as useWasmSceneEffects<br/>useWasmSceneEffects.ts:53
    participant BRIDGE as initSceneEffects<br/>scene-effects-bridge.ts:453
    participant WASM as scene_effects.js glue (built output)<br/>client/src/wasm/scene-effects
    participant RUST as ParticleField / EnergyWisps / AtmosphereField<br/>particles.rs:65, energy_wisps.rs:80, atmosphere.rs:72
    participant PFB as ParticleFieldBridge<br/>scene-effects-bridge.ts:84
    participant PARTINST as WasmParticleInstances useFrame<br/>WasmSceneEffects.tsx:200

    COMP->>HOOK: useWasmSceneEffects({particleCount, wispCount, atmosphereWidth,<br/>atmosphereHeight, enabled})
    HOOK->>BRIDGE: initSceneEffects() (module singleton, cachedAPI or initPromise)
    alt already cached or in-flight
        BRIDGE-->>HOOK: return cachedAPI or shared initPromise
    else first call
        BRIDGE->>WASM: dynamic import scene-effects/scene_effects.js, wasmModule.default()
        WASM-->>BRIDGE: initOutput.memory (WebAssembly.Memory)
        BRIDGE-->>HOOK: SceneEffectsAPI {createParticleField, createAtmosphereField,<br/>createWispField, version}
    end
    HOOK->>BRIDGE: api.createParticleField(particleCount), createAtmosphereField(w,h),<br/>createWispField(wispCount)
    BRIDGE->>RUST: new wasmModule.ParticleField(count) etc (wasm_bindgen constructor)
    BRIDGE->>PFB: new ParticleFieldBridge(inner, memory)
    HOOK-->>COMP: {ready:true, particles, atmosphere, wisps, update}

    rect rgb(232,244,250)
    Note over PARTINST,RUST: per frame -- WasmParticleInstances.useFrame (no explicit<br/>priority, default 0)
    PARTINST->>HOOK: update(dt clamped to 0.05, camera.x, camera.y, camera.z)
    HOOK->>PFB: particles.update(dt, camX, camY, camZ)
    PFB->>RUST: inner.update(dt, camera_x, camera_y, camera_z) -- particles.rs:109
    PARTINST->>PFB: getPositions(), getOpacities(), getSizes()
    PFB->>PFB: refreshViews() -- compare current byteLength/ptr/len vs cached
    alt unchanged since last frame (steady state, expected every frame)
        PFB-->>PARTINST: return cached Float32Array view (zero allocation)
    else WebAssembly.Memory grew or wasm-side buffer relocated
        PFB->>PFB: bounds check ptr+len*4 <= byteLength, throw if violated
        PFB->>PFB: logger.warn rebuilding views (byteLength changed, posPtr changed)
        PFB->>PFB: new Float32Array(memory.buffer, ptr, len) for positions/opacities/sizes
        Note over PFB: INVARIANT this IS the memory-growth view-invalidation hazard handled --<br/>every accessor calls refreshViews() before returning a view, so a stale view (from<br/>before Memory.grow detached the old ArrayBuffer) is never returned
    end
    loop for i in 0..min(count, mesh.count)
        PARTINST->>PARTINST: setMatrixAt(i, compose(position, identityQuat, scale=size*0.12))
        PARTINST->>PARTINST: instanceColor[i] = baseColor * opacity[i]
    end
    PARTINST->>PARTINST: instanceMatrix.needsUpdate = true, instanceColor.needsUpdate = true
    end

    rect rgb(252,238,230)
    Note over COMP,RUST: unmount -- useWasmSceneEffects cleanup (useWasmSceneEffects.ts:122-140)
    HOOK->>PFB: particles.dispose(), atmosphere.dispose(), wisps.dispose()
    PFB->>PFB: _disposed=true, views nulled
    PFB->>RUST: inner.free() (wasm_bindgen drop)
    end
    Note over RUST: get_positions_ptr/get_positions_len (particles.rs:189,194),<br/>get_pixels_ptr/get_pixels_len (atmosphere.rs:160,165), get_hues_ptr/get_hues_len<br/>(energy_wisps.rs:269,272) are the raw pointer/length exports underlying every<br/>Float32Array/Uint8Array view above
```

## VC-31.9 Transient agent-action beam -- store event to expiry
```mermaid
sequenceDiagram
    autonumber
    participant WS as handleAgentActionTagged<br/>binaryProtocol.ts:442
    participant STORE as transientBeamStore.pushBeams<br/>transientBeamStore.ts:67
    participant LAYER as TransientBeamsLayer<br/>TransientBeamsLayer.tsx:69
    participant HOOK as useTransientBeams<br/>useTransientBeams.ts:24
    participant MESH as TransientBeamMesh.updateBeam useFrame<br/>TransientBeamsLayer.tsx:166
    participant ENC as semanticEncoding agentActionColorHex/Shape<br/>semanticEncoding.ts:126,131

    Note over WS: 0x23 wire frame is a bare type tag, not a V4 header colon separated fields<br/>count:u16 then repeated len:u16 plus event bytes (binaryProtocol.ts:436-437). See VC-30<br/>for the surrounding websocket store and worker plumbing.
    WS->>STORE: pushTransientBeams(actions) exported non-React entry point<br/>(transientBeamStore.ts:115)
    STORE->>STORE: pushBeams -- clampDuration(durationMs) floor MIN_BEAM_DURATION_MS=400,<br/>default DEFAULT_BEAM_DURATION_MS=1500
    STORE->>STORE: beams.concat(incoming), FIFO trim to MAX_TRANSIENT_BEAMS=256 (oldest<br/>evicted first)

    LAYER->>HOOK: useTransientBeams() subscribes to beams array
    loop every frame (LAYER's own useFrame, no explicit priority)
        LAYER->>STORE: prune() calls pruneExpired()
        STORE->>STORE: filter beams where now - startTime < durationMs
        opt something expired
            STORE->>STORE: set({beams: alive}) -- only commits when count actually changed, avoids<br/>gratuitous re-render
        end
    end

    alt beams.length === 0
        LAYER-->>LAYER: return null (nothing rendered)
    else beams present
        LAYER->>MESH: mount one TransientBeamMesh per beam (React key=beam.id)
        MESH->>ENC: agentActionShape(actionType) -- radiusTop/radiusBottom/radialSegments per verb
        Note over ENC: Create widens into target, Delete narrows into it, Query thin probe, Link<br/>thick tie, Transform rounder beam. Colours QUERY blue UPDATE yellow CREATE green DELETE<br/>red LINK purple TRANSFORM cyan
        MESH->>MESH: CylinderGeometry(radiusTop, radiusBottom, height=1, radialSegments, 1,<br/>openEnded=true) built once in useMemo
        loop every frame (own useFrame, per beam mesh)
            MESH->>MESH: resolveAgentPosition(sourceAgentId), resolveNodePosition(targetNodeId)
            alt either resolver returns false
                MESH->>MESH: mesh.visible = false, skip silently (never throws)
            else both resolved
                MESH->>MESH: progress = (now - startTime) / durationMs
                MESH->>MESH: opacity = opacityEnvelope(progress) * maxOpacity -- fade-in 0..0.18, hold,<br/>fade-out 0.68..1
                alt opacity <= 0
                    MESH->>MESH: mesh.visible = false
                else visible
                    MESH->>MESH: compose midpoint, quaternion src-to-tgt direction, scale Y = distance
                    MESH->>MESH: material.opacity = opacity, mesh.visible = true
                end
            end
        end
    end
    Note over STORE,MESH: beam removed from the React tree once pruneExpired drops it from<br/>the store array -- React unmount runs the geometry.dispose()/material.dispose() cleanup<br/>(TransientBeamsLayer.tsx:159-164)
```

## VC-31.10 Frame budget instrumentation -- FPS auto-degrade and PerfProbe
```mermaid
sequenceDiagram
    autonumber
    participant R3F as R3F render loop
    participant FPS as useFpsMonitor.useFrame<br/>useFpsMonitor.ts:36
    participant SETSTORE as settingsStore.updateSettings<br/>useFpsMonitor.ts:117
    participant PP as PerfProbe.useFrame priority -10000<br/>PerfProbe.tsx:404
    participant CTRL as PerfController singleton<br/>PerfProbe.tsx:48, window.__perf

    R3F->>FPS: frame(state, delta) -- mounted inside GraphManager, GraphManager.tsx:132
    alt qualityGates.autoAdjust is false
        FPS->>FPS: reset internal tracking (samples, degradeLevel) but leave visual settings alone
    else autoAdjust enabled
        FPS->>FPS: fps = 1/delta, push to ring buffer, cap SAMPLE_SIZE=60 (about 1s at 60fps)
        alt fewer than 30 samples collected (SAMPLE_SIZE/2)
            FPS-->>R3F: wait for meaningful window
        else enough samples
            FPS->>FPS: avgFps = mean(samples)
            alt avgFps below minFpsThreshold (default 30) and degradeLevel < 3
                alt sustained below threshold for DEGRADE_SUSTAIN_MS=2000ms
                    FPS->>SETSTORE: updateSettings -- degradeLevel++, applyDegradeLevel(level)
                    Note over SETSTORE: level1 particleCount 128 atmosphereResolution 64. level2 wispsEnabled false particleCount 64. level3 sceneEffects.enabled false entirely
                end
            else avgFps recovered above threshold
                FPS->>FPS: reset lastDegradeTime = 0
            end
            alt avgFps above minFpsThreshold+10 (hysteresis band) and degradeLevel > 0
                alt sustained above band for RESTORE_SUSTAIN_MS=3000ms
                    FPS->>SETSTORE: updateSettings -- degradeLevel--, applyDegradeLevel(level)
                end
            end
        end
    end

    Note over PP: DEV-only, lazy-loaded (GraphCanvas.tsx:15), master-gated OFF by default -- zero overhead until window.__perf.on() or query perf=1 or localStorage perfProbe=1
    R3F->>PP: frame() -- priority -10000 runs FIRST every frame, before all other useFrame subscribers
    PP->>CTRL: controller.tick()
    alt controller.enabled false
        CTRL-->>PP: no-op, return immediately
    else armed
        CTRL->>CTRL: frameBuf ring buffer (FRAME_BUF=240 samples) records now-lastFrameTs
        CTRL->>CTRL: snapshot gl.info.render calls/triangles/programs, then gl.info.reset()
        opt now - lastRescan > 1000ms
            CTRL->>CTRL: rescanInstanced -- wrap any new InstancedMesh.setMatrixAt/setColorAt to count per-frame uploads
        end
        loop every reportIntervalMs=4000ms (setInterval, not useFrame)
            CTRL->>CTRL: report() -- console.table of frame p50/p90/p99, rAF callback costs, instanced upload counts, scene census
        end
    end
```
