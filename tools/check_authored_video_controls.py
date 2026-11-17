"""Original artifact controls over the real authored-video snapshot reader.

These controls write activity/source/PPM files from the retained independent
scene-inter reference. They do not launch or mock an App, decoder or process.
"""
from pathlib import Path
import tempfile
import unittest

import check_authored_video_harness as authored


ROOT = Path(__file__).resolve().parents[1]
STEM = ROOT / "tools/fixtures/scene-inter"


class AuthoredVideoControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.clip, cls.reference, _, _ = authored.fixture_reference(STEM)
        cls.images = {}

    @classmethod
    def ppm(cls, frame):
        """Render every pixel by nearest-neighbor scaling, not reader probes."""
        if frame not in cls.images:
            width, height = 480, 360
            source_width, source_height = cls.clip["width"], cls.clip["height"]
            rows = []
            for source_y in range(source_height):
                row = bytearray()
                for x in range(width):
                    source_x = x * source_width // width
                    offset = ((frame * source_height + source_y) * source_width + source_x) * 4
                    row.extend(cls.reference[offset:offset + 3])
                rows.append(bytes(row))
            payload = b"".join(rows[y * source_height // height] for y in range(height))
            cls.images[frame] = b"P6\n480 360\n255\n" + payload
        return cls.images[frame]

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="chezcat-authored-controls-")
        self.addCleanup(temporary.cleanup)
        self.out = Path(temporary.name)
        old_events = ("loadedmetadata", "loadeddata", "canplay", "play", "playing", "pause", "play", "playing", "ended")
        moving_events = old_events + ("play", "playing")
        self.states = {
            "ended": dict(playing=False, ended=True, time_ms=3000, frame=23,
                          generation=2, node=17, pending="None", error="None",
                          binding_paused=True, binding_seeking=False, events=old_events),
            "replay": dict(playing=True, ended=False, time_ms=253, frame=1,
                           generation=3, node=17, pending="Some(2)", error="None",
                           binding_paused=False, binding_seeking=False, events=moving_events),
            "replay-paused": dict(playing=False, ended=False, time_ms=300, frame=2,
                                  generation=3, node=17, pending="None", error="None",
                                  binding_paused=True, binding_seeking=False,
                                  events=moving_events + ("pause",)),
        }
        for stage in self.states:
            self.write_stage(stage)

    def write_stage(self, stage, *, prefix=None, pixels=None, **changes):
        state = dict(self.states[stage])
        state.update(changes)
        prefix = prefix or {"ended": "01", "replay": "02", "replay-paused": "03"}[stage]
        path = self.out / f"{prefix}-{stage}"
        boolean = lambda value: "true" if value else "false"
        activity = (
            "Stage paired_ppm_written=1\n0 error(s).\nauthored HTML video\n"
            f"HTML video binding node={state['node']} ready_state=4 network_state=1 width=64 height=48 "
            f"paused={boolean(state['binding_paused'])} ended={boolean(state['ended'])} "
            f"seeking={boolean(state['binding_seeking'])} current_ms={state['time_ms']} "
            "duration_ms=Some(3000) source_pending=false error=None\n"
            f"Media playing={boolean(state['playing'])} ended={boolean(state['ended'])} "
            f"time_ms={state['time_ms']} duration_ms=3000 frame={state['frame']} frames=24 "
            f"pending={state['pending']} canvas=64x48 generation={state['generation']} error={state['error']}\n"
            f"Media lifecycle dropped=0 {','.join(state['events'])}\n"
            "Media rendered rect=Some((0, 0, 480, 360))\n"
        )
        path.with_suffix(".activity.txt").write_text(activity, encoding="utf-8", newline="\n")
        path.with_suffix(".source.html").write_text(
            '<!doctype html><video controls src="scene-inter.ivf"></video>\n', encoding="utf-8")
        path.with_suffix(".layout.txt").write_text('text="Original video control"\n', encoding="utf-8")
        path.with_suffix(".ppm").write_bytes(self.ppm(state["frame"]) if pixels is None else pixels)

    def read(self):
        return authored.read_snapshots(self.out, STEM, self.clip, self.reference)

    def test_original_moving_frontier_and_settled_pause_are_both_required(self):
        checks, stages = self.read()
        keys = (
            "replay_all_snapshots_present", "replay_started_new_generation",
            "replay_pause_retained_source_generation", "replay_pause_after_moving_sample",
            "replay_frame_matches_original_timeline", "replay_moving_state",
            "replay_pending_matches_moving_time", "replay_no_decode_error",
            "replay-paused_no_pending_or_decode_error", "replay-paused_real_paused_binding",
            "replay-paused_frame_matches_frozen_time",
        )
        for key in keys:
            self.assertTrue(checks[key], key)
        for stage in ("ended", "replay", "replay-paused"):
            for suffix in ("zero_script_errors", "paired_pixels", "real_element_binding",
                           "actual_inline_rect", "independent_decoded_pixels"):
                self.assertTrue(checks[f"{stage}_{suffix}"], f"{stage}_{suffix}")
        self.assertEqual(stages["replay"]["pending"], "Some(2)")
        self.assertEqual(stages["replay"]["expected_frame_from_moving_time"], 2)
        self.assertEqual(stages["replay-paused"]["expected_frame_from_frozen_time"], 2)
        self.assertNotIn("real_replay", checks, "Unrelated all-stage checks are outside this reduced control")
        # The same independent moving sample can have no request in flight.
        self.write_stage("replay", pending="None")
        self.assertTrue(self.read()[0]["replay_pending_matches_moving_time"])

    def test_moving_decode_error_and_paused_pending_are_rejected_independently(self):
        self.write_stage("replay", error='Some("Original controlled decode failure")')
        checks, _ = self.read()
        self.assertFalse(checks["replay_no_decode_error"])
        self.assertTrue(checks["replay_independent_decoded_pixels"])
        self.write_stage("replay")
        self.write_stage("replay-paused", pending="Some(2)")
        checks, _ = self.read()
        self.assertFalse(checks["replay-paused_no_pending_or_decode_error"])
        self.assertTrue(checks["replay-paused_independent_decoded_pixels"])

    def test_moving_request_and_display_must_match_the_independent_timeline(self):
        for pending in ("Some(1)", "Some(3)"):
            with self.subTest(pending=pending):
                self.write_stage("replay", pending=pending)
                checks, _ = self.read()
                self.assertFalse(checks["replay_pending_matches_moving_time"])
                self.assertTrue(checks["replay_frame_matches_original_timeline"])
        for frame in (0, 3):
            with self.subTest(frame=frame):
                self.write_stage("replay", frame=frame, pending="None")
                checks, _ = self.read()
                self.assertFalse(checks["replay_frame_matches_original_timeline"])
                self.assertTrue(checks["replay_independent_decoded_pixels"])

    def test_replay_generation_and_paused_source_identity_cannot_change(self):
        for changes in (dict(node=18), dict(generation=4)):
            with self.subTest(stage="replay", changes=changes):
                self.write_stage("replay", **changes)
                self.assertFalse(self.read()[0]["replay_started_new_generation"])
        self.write_stage("replay")
        for changes in (dict(node=18), dict(generation=4)):
            with self.subTest(stage="replay-paused", changes=changes):
                self.write_stage("replay-paused", **changes)
                self.assertFalse(self.read()[0]["replay_pause_retained_source_generation"])

    def test_missing_or_ambiguous_replay_capture_is_rejected(self):
        for stage in ("replay", "replay-paused"):
            prefix = {"replay": "02", "replay-paused": "03"}[stage]
            with self.subTest(stage=stage, case="missing"):
                (self.out / f"{prefix}-{stage}.activity.txt").unlink()
                checks, stages = self.read()
                self.assertFalse(checks[f"{stage}_snapshot"])
                self.assertFalse(checks["replay_all_snapshots_present"])
                self.assertNotIn(stage, stages)
                self.write_stage(stage)
            with self.subTest(stage=stage, case="ambiguous"):
                self.write_stage(stage, prefix="99")
                checks, _ = self.read()
                self.assertFalse(checks[f"{stage}_snapshot"])
                self.assertFalse(checks["replay_all_snapshots_present"])
                (self.out / f"99-{stage}.activity.txt").unlink()

    def test_paused_pixels_and_timed_frame_remain_independent_obligations(self):
        self.write_stage("replay-paused", pixels=b"P6\n480 360\n255\n" + bytes(480 * 360 * 3))
        checks, _ = self.read()
        self.assertFalse(checks["replay-paused_independent_decoded_pixels"])
        self.assertTrue(checks["replay-paused_actual_inline_rect"])
        self.assertTrue(checks["replay-paused_frame_matches_frozen_time"])
        self.write_stage("replay-paused", frame=1, pixels=self.ppm(2))
        checks, _ = self.read()
        self.assertFalse(checks["replay-paused_frame_matches_frozen_time"])
        # Correct independent frame2 pixels cannot hide a stale frame report.
        self.assertTrue(checks["replay-paused_independent_decoded_pixels"])

    def test_moving_and_paused_binding_state_are_both_checked(self):
        self.write_stage("replay", binding_paused=True)
        self.assertFalse(self.read()[0]["replay_moving_state"])
        self.write_stage("replay")
        for changes in (dict(binding_paused=False), dict(binding_seeking=True)):
            with self.subTest(changes=changes):
                self.write_stage("replay-paused", **changes)
                self.assertFalse(self.read()[0]["replay-paused_real_paused_binding"])

    def test_pause_must_follow_the_moving_sample_without_restart_or_seek(self):
        moving = self.states["replay"]["events"]
        for events in (moving, moving + ("pause", "pause"), moving + ("pause", "playing")):
            with self.subTest(events=events):
                self.write_stage("replay-paused", events=events)
                self.assertFalse(self.read()[0]["replay_pause_after_moving_sample"])
        self.write_stage("replay-paused", time_ms=200, frame=1)
        self.assertFalse(self.read()[0]["replay_pause_after_moving_sample"])


if __name__ == "__main__":
    unittest.main()
