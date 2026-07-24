import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./env";

const TARGET_SAMPLE_RATE = 16_000;

let audioContext: AudioContext | null = null;
let processorNode: ScriptProcessorNode | null = null;
let sourceNode: MediaStreamAudioSourceNode | null = null;
let mediaStream: MediaStream | null = null;
let capturedSamples: number[] = [];

function downsampleBuffer(buffer: Float32Array, inputRate: number, outputRate: number): Float32Array {
  if (outputRate === inputRate) return buffer;
  const ratio = inputRate / outputRate;
  const newLength = Math.round(buffer.length / ratio);
  const result = new Float32Array(newLength);
  for (let i = 0; i < newLength; i++) {
    result[i] = buffer[Math.floor(i * ratio)];
  }
  return result;
}

export async function startCapture(): Promise<void> {
  if (isTauri) {
    await invoke("start_recording");
    return;
  }

  mediaStream = await navigator.mediaDevices.getUserMedia({ audio: { channelCount: 1 } });
  audioContext = new AudioContext();
  sourceNode = audioContext.createMediaStreamSource(mediaStream);
  // ScriptProcessorNode is deprecated in favor of AudioWorkletNode, but needs
  // no separate module file to load and is more than adequate for capturing
  // short wake-word/command clips.
  processorNode = audioContext.createScriptProcessor(4096, 1, 1);
  capturedSamples = [];

  processorNode.onaudioprocess = (event) => {
    const input = event.inputBuffer.getChannelData(0);
    const downsampled = downsampleBuffer(input, audioContext!.sampleRate, TARGET_SAMPLE_RATE);
    for (let i = 0; i < downsampled.length; i++) {
      capturedSamples.push(downsampled[i]);
    }
  };

  sourceNode.connect(processorNode);
  processorNode.connect(audioContext.destination);
}

export async function stopCapture(): Promise<number[]> {
  if (isTauri) {
    return invoke("stop_recording");
  }

  processorNode?.disconnect();
  sourceNode?.disconnect();
  mediaStream?.getTracks().forEach((track) => track.stop());
  await audioContext?.close();

  const samples = capturedSamples;
  capturedSamples = [];
  processorNode = null;
  sourceNode = null;
  mediaStream = null;
  audioContext = null;
  return samples;
}
