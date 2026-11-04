/** Static stand-ins for the editor panels. Real data arrives with the dubbing features. */
export interface SampleLine {
  start: string;
  end: string;
  text: string;
  voice: "male" | "female";
}

export const SAMPLE_LINES: SampleLine[] = [
  { start: "00:00.00", end: "00:09.86", text: "Sample line one.", voice: "female" },
  { start: "00:08.50", end: "00:18.32", text: "Sample line two.", voice: "male" },
  { start: "00:20.50", end: "00:25.61", text: "Sample line three.", voice: "male" },
  { start: "00:25.00", end: "00:29.37", text: "Sample line four.", voice: "male" },
  { start: "00:29.50", end: "00:36.27", text: "Sample line five.", voice: "female" },
];

/** Clip widths and offsets are percentages of the timeline, so the sample scales with the panel. */
export const SAMPLE_CLIPS = [
  { left: 0, width: 24 },
  { left: 27, width: 14 },
  { left: 42, width: 12 },
  { left: 56, width: 18 },
  { left: 78, width: 20 },
];
