
import { invoke } from "@tauri-apps/api/core";

export enum GpuVendor {
  NVIDIA = "NVIDIA",
  AMD = "AMD",
  Intel = "Intel",
  Unknown = "Unknown",
}

export interface CpuInfo {
  name: string;
  cores: number;
  threads: number;
  max_clock_speed: number;
  l3_cache_size: number;
  load_percentage: number | null;
}

export interface GpuInfo {
  name: string;
  vendor: GpuVendor;
  memory_gb: number;
  driver_version: string;
  temperature: number | null;
  usage: number | null;
}

export interface MemoryInfo {
  manufacturer: string;
  part_number: string;
  capacity_gb: number;
  speed_mhz: number;
  bank_label: string;
}

export interface HardwareInfo {
  cpu: CpuInfo;
  gpu: GpuInfo[];
  memory: MemoryInfo[];
  motherboard: string;
  disk: string[];
}

export async function getHardwareInfo(): Promise<HardwareInfo> {
  return await invoke<HardwareInfo>("get_hardware");
}
