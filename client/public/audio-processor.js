/**
 * AudioProcessor - Audio worklet for real-time audio processing
 * Handles audio level detection and optional processing
 */

class AudioProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();

    this.bufferSize = options.processorOptions?.bufferSize || 4096;
    this.sampleBuffer = [];
    this.frameCount = 0;

    // Audio level detection parameters
    this.smoothingFactor = 0.8;
    this.currentLevel = 0;
    this.levelUpdateRate = Math.floor(sampleRate / 60); // ~60fps updates
  }

  process(inputs, outputs, parameters) {
    const input = inputs[0];
    const output = outputs[0];

    // If no input, return early
    if (!input || input.length === 0) {
      return true;
    }

    const inputChannel = input[0];
    const outputChannel = output[0];

    // Pass audio through unchanged
    if (outputChannel) {
      outputChannel.set(inputChannel);
    }

    // Calculate audio level
    if (inputChannel && inputChannel.length > 0) {
      this.calculateAudioLevel(inputChannel);
    }

    this.frameCount++;
    return true;
  }

  calculateAudioLevel(samples) {
    // Calculate RMS (Root Mean Square) for audio level
    let sum = 0;
    for (let i = 0; i < samples.length; i++) {
      sum += samples[i] * samples[i];
    }

    const rms = Math.sqrt(sum / samples.length);

    // Smooth the level to avoid jittery updates
    this.currentLevel = this.smoothingFactor * this.currentLevel + (1 - this.smoothingFactor) * rms;

    // Send level updates at a reasonable rate
    if (this.frameCount % this.levelUpdateRate === 0) {
      this.port.postMessage({
        type: 'audioLevel',
        level: Math.min(this.currentLevel * 10, 1) // Scale and clamp to 0-1
      });
    }
  }
}

// Register the processor
registerProcessor('audio-processor', AudioProcessor);