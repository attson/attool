import unittest
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from psd_tools.constants import Tag

from psd_template_bridge import first_text_style, shape_layer_data


class BBox:
    x1 = 0
    y1 = 0
    x2 = 200
    y2 = 40
    width = 200
    height = 40


class TextLayer:
    bbox = BBox()
    resource_dict = {"FontSet": []}
    engine_dict = {
        "StyleRun": {
            "RunArray": [
                {
                    "StyleSheet": {
                        "StyleSheetData": {
                            "FontSize": 20,
                            "Tracking": -100,
                        }
                    }
                }
            ]
        }
    }


class TaggedBlocks:
    def __init__(self, data):
        self.data = data

    def get_data(self, tag):
        return self.data.get(tag)


class RectangleOrigin:
    pass


class RoundedRectangleOrigin:
    pass


RoundedRectangleOrigin.__name__ = "RoundedRectangle"
RectangleOrigin.__name__ = "Rectangle"


class StrokeEffect:
    enabled = True
    present = True
    shown = True
    size = 4
    color = {b"Rd  ": 255, b"Grn ": 205.8, b"Bl  ": 99}


StrokeEffect.__name__ = "Stroke"


class ShapeLayer:
    kind = "shape"
    origination = [RectangleOrigin()]
    effects = []

    def __init__(self, tagged_blocks):
        self.tagged_blocks = tagged_blocks


class PsdTemplateBridgeTest(unittest.TestCase):
    def test_converts_photoshop_tracking_to_letter_spacing_pixels(self):
        style = first_text_style(TextLayer())

        self.assertEqual(style["letterSpacing"], -2.0)

    def test_extracts_editable_shape_fill_and_effect_stroke(self):
        layer = ShapeLayer(
            TaggedBlocks(
                {
                    Tag.SOLID_COLOR_SHEET_SETTING: {b"Clr ": {b"Rd  ": 19.8, b"Grn ": 29.3, b"Bl  ": 55}},
                }
            )
        )
        layer.effects = [StrokeEffect()]

        self.assertEqual(
            shape_layer_data(layer),
            {"shape": "rect", "strokeWidth": 4.0, "fill": "#141d37", "stroke": "#ffce63"},
        )

    def test_respects_disabled_shape_fill_and_vector_stroke(self):
        layer = ShapeLayer(
            TaggedBlocks(
                {
                    Tag.VECTOR_STROKE_DATA: {
                        b"fillEnabled": False,
                        b"strokeEnabled": True,
                        b"strokeStyleLineWidth": 13.75,
                        b"strokeStyleContent": {b"Clr ": {b"Rd  ": 20, b"Grn ": 30, b"Bl  ": 55}},
                    },
                    Tag.VECTOR_STROKE_CONTENT_DATA: {b"Clr ": {b"Rd  ": 255, b"Grn ": 0, b"Bl  ": 0}},
                }
            )
        )

        self.assertEqual(shape_layer_data(layer), {"shape": "rect", "strokeWidth": 13.75, "stroke": "#141e37"})


if __name__ == "__main__":
    unittest.main()
