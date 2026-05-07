import unittest
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from psd_template_bridge import first_text_style


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


class PsdTemplateBridgeTest(unittest.TestCase):
    def test_converts_photoshop_tracking_to_letter_spacing_pixels(self):
        style = first_text_style(TextLayer())

        self.assertEqual(style["letterSpacing"], -2.0)


if __name__ == "__main__":
    unittest.main()
