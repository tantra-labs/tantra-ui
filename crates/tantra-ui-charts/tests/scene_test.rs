//! TDD tests for unified 6-layer `OverlayScene` and extended chart primitives (`tantra-ui-charts`).

use rust_decimal::Decimal;
use schemars::schema_for;
use tantra_ui_charts::axis::ChartRange;
use tantra_ui_charts::candle::GeometryError;
use tantra_ui_charts::levels::LevelLine;
use tantra_ui_charts::markers::{MarkerKind, MarkerSide, SignalMarker};
use tantra_ui_charts::scene::{
    HedgeLegSide, HedgeLevelOverlay, HedgeOptionKind, HintAnnotation, HintMetric, HintSeverity,
    OverlayScene, SceneLayerGroup, SceneLayerMask, TradeConnectorRay,
};
use tantra_ui_charts::series::{ChartPoint, ChartSeries};
use tantra_ui_charts::stops::{StopKind, StopLine};
use tantra_ui_charts::zones::ZoneBand;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

#[test]
fn extended_marker_kinds_round_trip_and_schema() {
    let cases = [
        (MarkerKind::Sweep, "sweep"),
        (MarkerKind::SweepSaved, "sweep_saved"),
        (MarkerKind::TargetHit, "target_hit"),
        (MarkerKind::ElephantBar, "elephant_bar"),
    ];
    for (kind, wire) in cases {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{wire}\""));
        let parsed: MarkerKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, kind);
    }

    let schema_json = serde_json::to_string(&schema_for!(MarkerKind)).unwrap();
    for wire in ["sweep", "sweep_saved", "target_hit", "elephant_bar"] {
        assert!(schema_json.contains(&format!("\"{wire}\"")));
    }
}

#[test]
fn hint_annotation_validates_price_and_anchors() {
    assert_eq!(
        HintAnnotation::new(
            2,
            dec("-1"),
            HintSeverity::Warning,
            "Invalid",
            "Negative price"
        ),
        Err(GeometryError::InvalidGeometry)
    );

    let hint = HintAnnotation::new(
        5,
        dec("75"),
        HintSeverity::Positive,
        "Sweep Saved (+2.4R)",
        "Wick probed below S1 outer edge but stayed above Outer SL.",
    )
    .unwrap()
    .with_metric("Penetration", "0.18x ATR")
    .with_metric("Outcome", "+2.40R");

    assert_eq!(
        hint.metrics,
        vec![
            HintMetric {
                key: "Penetration".into(),
                value: "0.18x ATR".into()
            },
            HintMetric {
                key: "Outcome".into(),
                value: "+2.40R".into()
            }
        ]
    );

    let r = ChartRange::new(dec("0"), dec("100"));
    assert_eq!(hint.anchor(&r, 101, 10), Ok((50, 25)));
}

#[test]
fn trade_connector_ray_validates_bounds_and_computes_endpoints() {
    assert_eq!(
        TradeConnectorRay::new(
            10,
            dec("100"),
            5,
            dec("110"),
            MarkerSide::Buy,
            dec("2"),
            false
        ),
        Err(GeometryError::InvalidGeometry)
    );
    assert_eq!(
        TradeConnectorRay::new(
            1,
            dec("-5"),
            5,
            dec("110"),
            MarkerSide::Buy,
            dec("2"),
            false
        ),
        Err(GeometryError::InvalidGeometry)
    );

    let ray = TradeConnectorRay::new(
        2,
        dec("25"),
        7,
        dec("75"),
        MarkerSide::Buy,
        dec("3.0"),
        false,
    )
    .unwrap()
    .with_label("+3R Target");

    let r = ChartRange::new(dec("0"), dec("100"));
    assert_eq!(ray.anchors(&r, 101, 10), Ok(((20, 75), (70, 25))));
    assert_eq!(ray.label.as_deref(), Some("+3R Target"));
}

#[test]
fn hedge_level_overlay_validates_and_maps_rows() {
    assert_eq!(
        HedgeLevelOverlay::new(
            "Short OTM Put",
            HedgeLegSide::Sell,
            HedgeOptionKind::Put,
            Decimal::ZERO,
            0,
            50,
        ),
        Err(GeometryError::InvalidGeometry)
    );
    assert_eq!(
        HedgeLevelOverlay::new(
            "Short OTM Put",
            HedgeLegSide::Sell,
            HedgeOptionKind::Put,
            dec("24100"),
            50,
            10,
        ),
        Err(GeometryError::InvalidGeometry)
    );

    let hedge = HedgeLevelOverlay::new(
        "Short OTM Put",
        HedgeLegSide::Sell,
        HedgeOptionKind::Put,
        dec("50"),
        10,
        40,
    )
    .unwrap()
    .with_breakeven(dec("48.5"))
    .with_max_loss(dec("12500"))
    .with_summary("Bull Put Credit Spread · 2-5 DTE");

    let r = ChartRange::new(dec("0"), dec("100"));
    assert_eq!(hedge.y_from_range(&r, 101), Ok(50));
    assert_eq!(hedge.breakeven, Some(dec("48.5")));
    assert_eq!(hedge.max_loss, Some(dec("12500")));
}

#[test]
fn scene_layer_mask_controls_all_six_groups() {
    let mut mask = SceneLayerMask::default();
    assert_eq!(mask, SceneLayerMask::all());
    for group in [
        SceneLayerGroup::Signals,
        SceneLayerGroup::Bands,
        SceneLayerGroup::Stops,
        SceneLayerGroup::Indicators,
        SceneLayerGroup::Hints,
        SceneLayerGroup::Hedges,
    ] {
        assert!(mask.is_visible(group));
        mask.toggle(group);
        assert!(!mask.is_visible(group));
        mask.set_visible(group, true);
        assert!(mask.is_visible(group));
    }
    assert!(!SceneLayerMask::none().is_visible(SceneLayerGroup::Hints));
}

#[test]
fn overlay_scene_computes_union_y_range_filters_layers_and_roundtrips_json_and_schema() {
    let mut scene = OverlayScene::new("elephant_edge_v3", "NSE:NIFTY50", "5m");
    assert_eq!(scene.y_range(), None);

    scene.signals.push(SignalMarker::new(
        dec("24200"),
        10,
        MarkerSide::Buy,
        MarkerKind::SweepSaved,
    ));
    scene.connectors.push(
        TradeConnectorRay::new(
            10,
            dec("24200"),
            18,
            dec("24500"),
            MarkerSide::Buy,
            dec("2.5"),
            false,
        )
        .unwrap(),
    );
    scene.bands.push(
        ZoneBand::new(dec("24120"), dec("24180"), 0, 20)
            .unwrap()
            .with_label("S1"),
    );
    scene.levels.push(
        LevelLine::new(dec("24300"), 0, 20)
            .unwrap()
            .with_label("Eq"),
    );
    scene
        .stops
        .push(StopLine::new(dec("24080"), 10, 20, StopKind::Initial).unwrap());
    scene.indicators.push(ChartSeries::new(
        "ATR Envelope",
        vec![ChartPoint::new(Decimal::ZERO, dec("24350"))],
    ));
    scene.hints.push(
        HintAnnotation::new(
            10,
            dec("24110"),
            HintSeverity::Positive,
            "Outer SL Shield Survived",
            "Tight SL was hunted at 24130 while Outer SL 24080 held.",
        )
        .unwrap(),
    );
    scene.hedges.push(
        HedgeLevelOverlay::new(
            "Short PE 24100",
            HedgeLegSide::Sell,
            HedgeOptionKind::Put,
            dec("24050"),
            10,
            20,
        )
        .unwrap(),
    );

    let counts = scene.layer_counts();
    assert_eq!(counts.signals, 1);
    assert_eq!(counts.bands, 2); // 1 band + 1 level
    assert_eq!(counts.stops, 1);
    assert_eq!(counts.indicators, 1);
    assert_eq!(counts.hints, 1);
    assert_eq!(counts.hedges, 1);

    let full_range = scene.y_range().unwrap();
    assert_eq!(full_range.min, dec("24050"));
    assert_eq!(full_range.max, dec("24500"));

    scene.layers.set_visible(SceneLayerGroup::Hedges, false);
    scene.layers.set_visible(SceneLayerGroup::Signals, false);
    let visible = scene.filtered_visible();
    assert!(visible.signals.is_empty());
    assert!(visible.connectors.is_empty());
    assert!(visible.hedges.is_empty());
    assert_eq!(visible.stops.len(), 1);
    assert_eq!(visible.hints.len(), 1);
    let vis_range = visible.y_range().unwrap();
    assert_eq!(vis_range.min, dec("24080"));
    assert_eq!(vis_range.max, dec("24350"));

    let json = serde_json::to_string(&scene).unwrap();
    let roundtrip: OverlayScene = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip, scene);

    let schema_str = serde_json::to_string(&schema_for!(OverlayScene)).unwrap();
    assert!(schema_str.contains("OverlayScene"));
    assert!(schema_str.contains("HintAnnotation"));
    assert!(schema_str.contains("HedgeLevelOverlay"));
}
