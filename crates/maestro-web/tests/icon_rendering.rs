//! SVG geometry and real native rasterization through public functions.
#![cfg(test)]
use indexmap::IndexMap;
use maestro_web::icons::{IconNode, icons};
use serde::Deserialize;

/// Captured icon geometry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Geometry {
    /// Public catalog name.
    name: String,
    /// Ordered primitive nodes.
    nodes: IconNode,
}
#[test]
fn icon_catalog_preserves_each_geometry() {
    let expected: Vec<Geometry> =
        serde_json::from_str(include_str!("fixtures/icons.json")).unwrap();
    let actual = icons().unwrap();
    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.iter().map(|g| &g.name).collect::<Vec<_>>()
    );
    for geometry in expected {
        let nodes = &actual[&geometry.name];
        assert_eq!(nodes.len(), geometry.nodes.len());
        for ((tag, attrs), (expected_tag, expected_attrs)) in nodes.iter().zip(&geometry.nodes) {
            assert_eq!(tag, expected_tag);
            assert_eq!(
                attrs.iter().collect::<Vec<_>>(),
                expected_attrs.iter().collect::<Vec<_>>()
            );
        }
    }
}

/// Captured DOM node semantics, not serialization bytes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tree {
    /// Element local name.
    tag: String,
    /// Ordered literal attributes.
    attrs: IndexMap<String, String>,
    /// Ordered element children.
    children: Vec<Self>,
}
/// Size/class operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TreeInput {
    /// Catalog entry.
    name: String,
    /// Optional explicit size.
    size: Option<String>,
    /// Optional literal class suffix.
    class_name: Option<String>,
}
/// Captured element construction.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TreeCase {
    /// Supplied operands.
    input: TreeInput,
    /// Complete element semantics.
    expected: Tree,
}
/// Decode each supported size without silently defaulting an unknown spelling.
fn size(value: Option<&str>) -> maestro_web::icons::IconSize {
    use maestro_web::icons::IconSize;
    match value {
        None => IconSize::default(),
        Some("xs") => IconSize::Xs,
        Some("sm") => IconSize::Sm,
        Some("md") => IconSize::Md,
        Some("lg") => IconSize::Lg,
        Some("xl") => IconSize::Xl,
        _ => panic!("unknown size"),
    }
}
/// Compare XML's real decoded literals and child order with captured DOM values.
fn assert_tree(node: usvg::roxmltree::Node<'_, '_>, expected: &Tree) {
    assert_eq!(node.tag_name().name(), expected.tag);
    if let Some(namespace) = expected.attrs.get("xmlns") {
        assert_eq!(node.tag_name().namespace(), Some(namespace.as_str()));
    }
    let attrs: Vec<_> = node.attributes().map(|a| (a.name(), a.value())).collect();
    let expected_attrs: Vec<_> = expected
        .attrs
        .iter()
        .filter(|(k, _)| *k != "xmlns")
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(attrs, expected_attrs);
    let children: Vec<_> = node
        .children()
        .filter(usvg::roxmltree::Node::is_element)
        .collect();
    assert_eq!(children.len(), expected.children.len());
    for (child, expected) in children.into_iter().zip(&expected.children) {
        assert_tree(child, expected);
    }
}
#[test]
fn icon_sizes_and_classes_preserve_svg_attributes() {
    let cases: Vec<TreeCase> =
        serde_json::from_str(include_str!("fixtures/iconTrees.json")).unwrap();
    let catalog = icons().unwrap();
    for case in cases {
        let svg = maestro_web::icons::icon_svg(
            &catalog[&case.input.name],
            size(case.input.size.as_deref()),
            case.input.class_name.as_deref(),
        );
        let document = usvg::roxmltree::Document::parse(&svg).unwrap();
        assert_tree(document.root_element(), &case.expected);
    }
}

/// Caller-defined primitive observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomCase {
    /// Ordered typed primitives.
    input: IconNode,
    /// Captured DOM result.
    expected: Tree,
}
#[test]
fn icon_svg_escapes_attribute_values_before_parsing() {
    let cases: Vec<CustomCase> =
        serde_json::from_str(include_str!("fixtures/customIcons.json")).unwrap();
    for case in cases {
        let svg = maestro_web::icons::icon_svg(
            &case.input,
            maestro_web::icons::IconSize::default(),
            None,
        );
        let document = usvg::roxmltree::Document::parse(&svg).unwrap();
        assert_tree(document.root_element(), &case.expected);
    }
    let literal = "x\"'<&💡a\tb\nc\rd";
    let node = vec![(
        "circle".into(),
        IndexMap::from([(
            "data-label".into(),
            maestro_web::icons::SvgValue::Text(literal.into()),
        )]),
    )];
    let svg = maestro_web::icons::icon_svg(&node, maestro_web::icons::IconSize::Md, Some(literal));
    let document = usvg::roxmltree::Document::parse(&svg).unwrap();
    let root = document.root_element();
    assert_eq!(
        root.attribute("class"),
        Some(format!("w-5 h-5 {literal}").as_str())
    );
    let children: Vec<_> = root
        .children()
        .filter(usvg::roxmltree::Node::is_element)
        .collect();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].attributes().len(), 1);
    assert_eq!(children[0].attribute("data-label"), Some(literal));
}

#[test]
fn explicitly_bound_svg_primitives_rasterize() {
    use maestro_web::icons::{IconSize, SvgValue, icon, icon_svg};
    let node = vec![(
        "svg:circle".into(),
        IndexMap::from([
            ("cx".into(), SvgValue::Number(12.0)),
            ("cy".into(), SvgValue::Number(12.0)),
            ("r".into(), SvgValue::Number(5.0)),
            (
                "xmlns:svg".into(),
                SvgValue::Text("http://www.w3.org/2000/svg".into()),
            ),
        ]),
    )];
    let svg = icon_svg(&node, IconSize::Md, None);
    let document = usvg::roxmltree::Document::parse(&svg).unwrap();
    let child = document.root_element().first_element_child().unwrap();
    assert_eq!(child.tag_name().name(), "circle");
    assert_eq!(
        child.tag_name().namespace(),
        Some("http://www.w3.org/2000/svg")
    );
    let image = icon(&node, IconSize::Md, egui::Color32::WHITE, 1.0).unwrap();
    assert!(image.pixels.iter().any(|p| p.a() > 0));
}

#[test]
fn undeclared_svg_prefixes_return_parser_errors() {
    use maestro_web::icons::{IconSize, SvgValue, icon};
    for (tag, attrs) in [
        ("svg:circle", IndexMap::new()),
        (
            "use",
            IndexMap::from([("xlink:href".into(), SvgValue::Text("#shape".into()))]),
        ),
    ] {
        let node = vec![(tag.into(), attrs)];
        let error = icon(&node, IconSize::Md, egui::Color32::WHITE, 1.0).unwrap_err();
        assert!(std::error::Error::source(&error).is_some());
    }
}

#[test]
fn icon_raster_uses_requested_size_density_and_color() {
    use maestro_theme::BrandMode;
    use maestro_web::icons::{IconSize, icon};
    let pack = crate::support::brand::pack("../../assets/brand/brand.json").unwrap();
    let mut colors: Vec<_> = [BrandMode::Dark, BrandMode::Light]
        .into_iter()
        .map(|mode| {
            let presentation = pack.presentation(mode).unwrap();
            egui::Color32::from_hex(presentation.colors["text"]).unwrap()
        })
        .collect();
    colors.push(egui::Color32::from_rgba_unmultiplied(31, 117, 203, 128));
    let catalog = icons().unwrap();
    for (size, points) in [
        (IconSize::Xs, 12_u8),
        (IconSize::Sm, 16),
        (IconSize::Md, 20),
        (IconSize::Lg, 24),
        (IconSize::Xl, 32),
    ] {
        for density in [1.0_f32, 1.25, 1.5, 2.0] {
            for color in &colors {
                let image = icon(&catalog["Check"], size, *color, density).unwrap();
                let pixels =
                    num_traits::ToPrimitive::to_usize(&(f32::from(points) * density).ceil())
                        .unwrap();
                assert_eq!(image.size, [pixels, pixels]);
                assert!(image.pixels.iter().any(|p| p.a() == 0));
                let painted = image.pixels.iter().max_by_key(|p| p.a()).unwrap();
                assert!(painted.a() > 0);
                assert!(painted.a() <= color.a());
                let actual = painted.to_srgba_unmultiplied();
                let expected = color.to_srgba_unmultiplied();
                assert!((0..3).all(|channel| actual[channel].abs_diff(expected[channel]) <= 2));
            }
        }
    }
}
#[test]
fn icon_raster_handles_empty_and_custom_geometry() {
    use maestro_web::icons::{IconSize, SvgValue, icon};
    let color = egui::Color32::WHITE;
    let empty = icon(&vec![], IconSize::Md, color, 1.0).unwrap();
    assert!(empty.pixels.iter().all(|p| p.a() == 0));
    let circle = vec![(
        "circle".into(),
        IndexMap::from([
            ("cx".into(), SvgValue::Number(12.0)),
            ("cy".into(), SvgValue::Number(12.0)),
            ("r".into(), SvgValue::Number(5.0)),
        ]),
    )];
    let custom = icon(&circle, IconSize::Md, color, 1.0).unwrap();
    let check = icon(&icons().unwrap()["Check"], IconSize::Md, color, 1.0).unwrap();
    assert!(custom.pixels.iter().any(|p| p.a() > 0));
    assert_ne!(custom.pixels, check.pixels);
    assert!(icon(&circle, IconSize::Md, color, f32::NAN).is_err());
    assert!(icon(&circle, IconSize::Md, color, 0.0).is_err());
    let invalid = vec![("<bad".into(), IndexMap::new())];
    let error = icon(&invalid, IconSize::Md, color, 1.0).unwrap_err();
    assert!(std::error::Error::source(&error).is_some());
}
