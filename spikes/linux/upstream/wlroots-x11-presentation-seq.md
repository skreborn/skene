# backend/x11: presentation feedback for a commit arrives only with the next commit's present

**Project:** wlroots (seen in 0.17.1, as sway 1.9 ships it; master's X11 backend still presents with `commit_seq` unchanged, though whether the increment moved there was not checked)

## Summary

On the X11 backend, `wp_presentation_feedback.presented` for a surface commit is sent only when the
output presents the *next* frame. When a client stops committing (an animation's last frame), the
feedback for its last frame is never sent: not presented, not discarded.

## Steps to reproduce

1. Run sway 1.9 nested in an X server: `WLR_BACKENDS=x11 WLR_RENDERER=pixman sway` (reproduced with
   Xorg's dummy driver; Present uses its fake vblank there).
2. Run a client that commits a short animation (some 25 frames), asking for
   `wp_presentation.feedback` on each commit, and then stops committing.
3. Count the feedback events.

## Expected

Every commit gets `presented` (or `discarded`) about one refresh later, as on the headless backend.

## Actual

Each `presented` event arrives one output frame late, carrying the following frame's timestamp. The
last commit's feedback never arrives. We waited 5 s after the last commit and nothing came.
Traced client side:

```text
[frame] 28 ...
[wayland] frame 26 presented
[frame] 29 ...
[wayland] frame 27 presented
[frame] 30 ...
[wayland] frame 28 presented
[wayland] frame 29 presented
            (frame 30: nothing, ever)
```

## Cause (from reading the 0.17.1 source)

- `backend/x11/output.c` presents with `serial = output->wlr_output.commit_seq`, read before
  `wlr_output_commit_state` increments it (`types/output/output.c`: `output->commit_seq++` follows
  `impl->commit`). `handle_x11_present_event` then reports `.commit_seq = complete_notify->serial`.
- `types/wlr_presentation_time.c` records the feedback's `output_commit_seq` in the output's
  `commit` signal, which fires after the increment. It sends the feedback only for a present event
  whose `commit_seq` is equal to that number.
- The present event for commit *N* therefore carries *N − 1* and never matches. The present event
  for commit *N + 1* matches commit *N*'s feedback by accident.
- The headless backend sends `.commit_seq = wlr_output->commit_seq + 1` (`backend/headless/output.c`),
  which is why it does not show the problem.

## Suggested fix

Present with `serial = output->wlr_output.commit_seq + 1` in `output_commit_buffer`, and in the
`present_notify_msc` path as well, matching the headless backend. Another option is to report
`complete_notify->serial + 1`.
