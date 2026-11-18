//! How the timeline looks: the transport bar, the track, and what the
//! transport bar leaves out in narrow windows. Colours are the theme's
//! tokens (see [`crate::theme`]); the track's offsets are the constants in
//! [`super::draw`].

/// The timeline's rules.
pub(crate) const SHEET: &str = zgui::css!(
    "/* The timeline panel: the transport bar above the track. */

    .timeline {
        height: 104px; /* timeline::HEIGHT */
        flex-shrink: 0;
        background-color: var(--zui-color-card);
        color: var(--zui-color-card-foreground);
        border-top: 1px solid var(--zui-color-border);
    }

    .transport {
        height: 44px;
        flex-shrink: 0;
        align-items: center;
        gap: 10px;
        padding: 0 12px;
        border-bottom: 1px solid var(--zui-color-border);
        overflow: hidden;
    }

    /* Nothing in the bar wraps: what doesn't fit is shortened or, below
       the widths at the end of this sheet, left out, least needed first. */
    .transport__buttons, .transport__controls {
        flex-shrink: 0;
        align-items: center;
        gap: 2px;
    }

    .transport__play {
        margin: 0 4px;
    }

    .timecode {
        flex-shrink: 0;
        margin-left: 8px;
        white-space: nowrap;
        font-size: 17px;
        font-variant-numeric: tabular-nums;
    }

    .timecode--total {
        margin-left: 0;
        font-size: 13px;
        color: var(--zui-color-muted-foreground);
    }

    .transport__spacer {
        flex-grow: 1;
        min-width: 0;
    }

    .transport__meta {
        min-width: 0;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        font-size: 12px;
        font-variant-numeric: tabular-nums;
        color: var(--zui-color-muted-foreground);
    }

    /* The track: the ruler, the lane, and two canvases over them. Their
       offsets are the constants in `timeline::draw`. */

    .track {
        position: relative;
        flex-grow: 1;
        margin: 0 20px;
    }

    .ruler {
        position: absolute;
        left: 0;
        right: 0;
        top: 0;
        height: 26px; /* RULER */
        overflow: hidden;
    }

    .tick__label {
        position: absolute;
        left: var(--mg-at, 0%);
        bottom: 6px;
        margin-left: 4px;
        font-size: 10px;
        font-variant-numeric: tabular-nums;
        color: var(--zui-color-muted-foreground);
    }

    .lane {
        position: absolute;
        left: 0;
        right: 0;
        top: 30px; /* LANE_TOP */
        bottom: 12px; /* LANE_BOTTOM */
        border-radius: 4px;
        background-color: var(--zui-color-muted);
    }

    /* The canvases fill with their own text colour at varying opacity:
       ticks, frame stripes and the navigator in the muted text colour,
       the playhead and the played range in the accent. A canvas keeps its
       default 300 x 150 unless it is given a size, so insets alone
       wouldn't stretch it over the track. */

    .marks, .playhead {
        position: absolute;
        left: 0;
        top: 0;
        width: 100%;
        height: 100%;
    }

    .marks {
        color: var(--zui-color-muted-foreground);
    }

    .playhead {
        color: var(--zui-color-primary);
    }

    /* Narrow windows */

    @media (max-width: 720px) {
        .transport__meta { display: none; }
    }

    @media (max-width: 560px) {
        .timecode--total { display: none; }
    }

    @media (max-width: 440px) {
        .transport .transport__jump { display: none; }
        .transport { gap: 6px; padding: 0 8px; }
        .track { margin: 0 12px; }
    }"
);
