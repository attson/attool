#!/usr/bin/env python3
import argparse
import json
import re
import sys
import uuid
from datetime import datetime
from pathlib import Path

from psd_tools import PSDImage
from psd_tools.constants import Tag

BINDING_RE = re.compile(r"\{\{\s*([a-zA-Z][a-zA-Z0-9_]*)\s*\}\}")
SUGGESTED_BINDINGS = {
    "商品图": "product_image",
    "LOGO": "logo",
    "大标题": "title",
    "右下角标题": "install_note",
    "户外/家用/店铺/办公": "usage_text",
    "提带设计 单手提拿": "feature_text",
}


def now_text():
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def binding_for(name):
    match = BINDING_RE.search(name)
    if match:
        return match.group(1)
    return SUGGESTED_BINDINGS.get(name)


def bbox_values(layer):
    bbox = layer.bbox
    if hasattr(bbox, "x1"):
        return bbox.x1, bbox.y1, bbox.x2, bbox.y2, bbox.width, bbox.height
    x1, y1, x2, y2 = bbox
    return x1, y1, x2, y2, max(0, x2 - x1), max(0, y2 - y1)


def rgba_to_hex(values, fallback="#111111"):
    if not values or len(values) < 4:
        return fallback
    rgb = values[1:4]
    return "#" + "".join(f"{max(0, min(255, round(channel * 255))):02x}" for channel in rgb)


def rgb_dict_to_hex(values, fallback=None):
    if not values:
        return fallback
    channels = []
    for key in (b"Rd  ", b"Grn ", b"Bl  "):
        if key not in values:
            return fallback
        channels.append(values[key])
    return "#" + "".join(f"{max(0, min(255, round(float(channel)))):02x}" for channel in channels)


def descriptor_get(data, key, default=None):
    if not data:
        return default
    keys = (key, key.encode("utf-8") if isinstance(key, str) else key.decode("utf-8", errors="ignore"))
    for candidate in keys:
        try:
            if candidate in data:
                return data[candidate]
        except TypeError:
            continue
    return default


def tagged_data(layer, tag):
    try:
        return layer.tagged_blocks.get_data(tag)
    except Exception:
        return None


def shape_kind_from_layer(layer):
    for origin in getattr(layer, "origination", []) or []:
        name = type(origin).__name__
        if name == "RoundedRectangle":
            return "roundRect"
        if name == "Ellipse":
            return "ellipse"
        if name == "Rectangle":
            return "rect"

    origin_data = tagged_data(layer, Tag.VECTOR_ORIGINATION_DATA)
    descriptors = descriptor_get(origin_data, "keyDescriptorList", []) or []
    for descriptor in descriptors:
        origin_type = descriptor_get(descriptor, "keyOriginType")
        if origin_type == 2:
            return "roundRect"
        if origin_type == 5:
            return "ellipse"
        if origin_type == 1:
            return "rect"
    return "rect"


def rounded_rect_radius(layer):
    origin_data = tagged_data(layer, Tag.VECTOR_ORIGINATION_DATA)
    descriptors = descriptor_get(origin_data, "keyDescriptorList", []) or []
    for descriptor in descriptors:
        radii = descriptor_get(descriptor, "keyOriginRRectRadii")
        if not radii:
            continue
        values = [float(value) for key, value in radii.items() if key != b"unitValueQuadVersion"]
        if values:
            return max(values)
    return None


def solid_shape_fill(layer):
    solid = tagged_data(layer, Tag.SOLID_COLOR_SHEET_SETTING)
    color = descriptor_get(solid, b"Clr ")
    if color:
        return rgb_dict_to_hex(color)

    content = tagged_data(layer, Tag.VECTOR_STROKE_CONTENT_DATA)
    color = descriptor_get(content, b"Clr ")
    if color:
        return rgb_dict_to_hex(color)
    return None


def vector_stroke(layer):
    stroke_data = tagged_data(layer, Tag.VECTOR_STROKE_DATA)
    if not descriptor_get(stroke_data, "strokeEnabled", False):
        return None, None

    content = descriptor_get(stroke_data, "strokeStyleContent")
    color = descriptor_get(content, b"Clr ")
    return rgb_dict_to_hex(color), float(descriptor_get(stroke_data, "strokeStyleLineWidth", 0.0) or 0.0)


def effect_stroke(layer):
    for effect in getattr(layer, "effects", []) or []:
        if type(effect).__name__ != "Stroke":
            continue
        if not (getattr(effect, "enabled", False) and getattr(effect, "present", False) and getattr(effect, "shown", True)):
            continue
        return rgb_dict_to_hex(getattr(effect, "color", None)), float(getattr(effect, "size", 0.0) or 0.0)
    return None, None


def shape_layer_data(layer):
    stroke_data = tagged_data(layer, Tag.VECTOR_STROKE_DATA)
    fill_enabled = True if stroke_data is None else bool(descriptor_get(stroke_data, "fillEnabled", True))
    fill = solid_shape_fill(layer) if fill_enabled else None
    stroke, stroke_width = vector_stroke(layer)
    if not stroke:
        stroke, stroke_width = effect_stroke(layer)

    data = {
        "shape": shape_kind_from_layer(layer),
        "strokeWidth": stroke_width or 0,
    }
    if fill:
        data["fill"] = fill
    if stroke and stroke_width:
        data["stroke"] = stroke
    if data["shape"] == "roundRect":
        radius = rounded_rect_radius(layer)
        if radius is not None:
            data["radius"] = radius
    return data


def photoshop_tracking_to_letter_spacing(tracking, font_size):
    return float(tracking) / 1000.0 * float(font_size)


def first_text_style(layer):
    result = {
        "fontFamily": "PingFang SC",
        "fontSize": max(12, bbox_values(layer)[5] or 24),
        "fontWeight": 700,
        "color": "#111111",
        "align": "left",
    }
    try:
        engine = layer.engine_dict
        resources = layer.resource_dict
        font_set = resources.get("FontSet", [])
        run_array = engine.get("StyleRun", {}).get("RunArray", [])
        if run_array:
            data = run_array[0].get("StyleSheet", {}).get("StyleSheetData", {})
            font_index = data.get("Font")
            if isinstance(font_index, int) and font_index < len(font_set):
                result["fontFamily"] = str(font_set[font_index].get("Name", result["fontFamily"]))
            if "FontSize" in data:
                result["fontSize"] = float(data["FontSize"])
            if "FillColor" in data:
                result["color"] = rgba_to_hex(data["FillColor"].get("Values"))
            if "Tracking" in data:
                result["letterSpacing"] = photoshop_tracking_to_letter_spacing(data["Tracking"], result["fontSize"])
            if "Leading" in data and not data.get("AutoLeading", False):
                leading = float(data["Leading"])
                if leading > 0:
                    result["lineHeight"] = leading
            if "StrokeColor" in data:
                result["strokeColor"] = rgba_to_hex(data["StrokeColor"].get("Values"))
                result["strokeWidth"] = 1.0
    except Exception:
        pass
    return result


def detect_text_orientation(layer, text, font_size):
    try:
        engine = layer.engine_dict
        editor = engine.get("EngineDict", {}).get("Editor") if isinstance(engine, dict) else None
        if isinstance(editor, dict):
            for key in ("Orientation", "Orntn", "TextOrientation"):
                value = editor.get(key)
                if isinstance(value, (int, float)) and int(value) == 1:
                    return "vertical"
                if isinstance(value, str) and value.lower().startswith("vert"):
                    return "vertical"
    except Exception:
        pass
    if text and len(text) >= 2 and font_size and font_size > 0:
        _, _, _, _, width, height = bbox_values(layer)
        if width > 0 and height >= width * 1.5:
            return "vertical"
    return None


def save_layer_png(layer, asset_dir):
    image = layer.composite()
    if image is None:
        return None
    asset_id = f"asset-{uuid.uuid4().hex}"
    output = asset_dir / f"{asset_id}.png"
    image.save(output)
    return {
        "id": asset_id,
        "name": layer.name,
        "path": str(output),
        "sourceLayerId": None,
        "mimeType": "image/png",
        "width": image.width,
        "height": image.height,
    }


def layer_to_template(layer, asset_dir):
    x1, y1, _x2, _y2, width, height = bbox_values(layer)
    layer_id = f"layer-{uuid.uuid4().hex}"
    base = {
        "id": layer_id,
        "name": layer.name,
        "type": "group" if layer.is_group() else "image",
        "x": float(x1),
        "y": float(y1),
        "width": float(max(1, width)),
        "height": float(max(1, height)),
        "visible": bool(layer.visible),
        "opacity": float(getattr(layer, "opacity", 255)) / 255.0,
        "rotation": 0,
        "bindingKey": binding_for(layer.name),
        "locked": False,
    }

    assets = []
    if layer.is_group():
        children = []
        for child in layer:
            child_layer, child_assets = layer_to_template(child, asset_dir)
            children.append(child_layer)
            assets.extend(child_assets)
        base["children"] = children
        return base, assets

    if layer.kind == "type":
        style = first_text_style(layer)
        base["type"] = "text"
        text_data = {
            "text": layer.text or "",
            "fontFamily": style["fontFamily"],
            "fontSize": style["fontSize"],
            "fontWeight": style["fontWeight"],
            "color": style["color"],
            "align": style["align"],
        }
        for optional_key in ("letterSpacing", "lineHeight", "strokeColor", "strokeWidth"):
            if optional_key in style:
                text_data[optional_key] = style[optional_key]
        orientation = detect_text_orientation(layer, text_data["text"], text_data["fontSize"])
        if orientation:
            text_data["orientation"] = orientation
        base["text"] = text_data
        return base, assets

    if layer.kind == "shape":
        base["type"] = "shape"
        base["shape"] = shape_layer_data(layer)
        return base, assets

    asset = save_layer_png(layer, asset_dir)
    if asset:
        asset["sourceLayerId"] = layer_id
        assets.append(asset)
        base["type"] = "image"
        base["image"] = {
            "assetId": asset["id"],
            "fit": "stretch",
            "replaceable": bool(base.get("bindingKey")) or layer.kind in ("smartobject", "pixel"),
        }
        return base, assets

    base["type"] = "shape"
    base["shape"] = {"shape": "rect", "strokeWidth": 0}
    return base, assets


def import_psd(psd_path, output_dir):
    psd = PSDImage.open(psd_path)
    if str(psd.color_mode).split(".")[-1] != "RGB" or psd.depth != 8:
        raise ValueError("当前只支持 RGB / 8-bit PSD")

    template_id = f"tpl-{uuid.uuid4().hex}"
    template_dir = output_dir / template_id
    asset_dir = template_dir / "assets"
    asset_dir.mkdir(parents=True, exist_ok=True)

    preview_path = template_dir / "preview.png"
    psd.composite().save(preview_path)

    layers = []
    assets = []
    for layer in psd:
        converted, layer_assets = layer_to_template(layer, asset_dir)
        layers.append(converted)
        assets.extend(layer_assets)

    timestamp = now_text()
    return {
        "id": template_id,
        "name": Path(psd_path).stem,
        "canvasWidth": psd.width,
        "canvasHeight": psd.height,
        "layers": layers,
        "assets": assets,
        "sourcePsdPath": str(psd_path),
        "previewPath": str(preview_path),
        "createdAt": timestamp,
        "updatedAt": timestamp,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--psd", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    result = import_psd(Path(args.psd), Path(args.output_dir))
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(json.dumps({"error": str(exc)}, ensure_ascii=False), file=sys.stderr)
        sys.exit(1)
