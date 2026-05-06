#!/usr/bin/env python3
import argparse
import json
import re
import sys
import uuid
from datetime import datetime
from pathlib import Path

from psd_tools import PSDImage

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
    except Exception:
        pass
    return result


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
        base["text"] = {
            "text": layer.text or "",
            "fontFamily": style["fontFamily"],
            "fontSize": style["fontSize"],
            "fontWeight": style["fontWeight"],
            "color": style["color"],
            "align": style["align"],
        }
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
    base["shape"] = {"shape": "rect", "fill": "rgba(0,0,0,0)", "strokeWidth": 0}
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
