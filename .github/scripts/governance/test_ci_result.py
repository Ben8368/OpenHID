import unittest

from ci_result import summarize


class RequiredCheckTests(unittest.TestCase):
    def test_all_success(self):
        report, passed = summarize({"rust": {"result": "success"}, "policy": {"result": "success"}})
        self.assertTrue(passed)
        self.assertIn("GREEN", report)

    def test_no_silent_success(self):
        for result in ("failure", "cancelled", "skipped", None):
            with self.subTest(result=result):
                report, passed = summarize({"rust": {"result": result}, "policy": {"result": "success"}})
                self.assertFalse(passed)
                self.assertIn("RED", report)
        self.assertFalse(summarize({})[1])


if __name__ == "__main__":
    unittest.main()
