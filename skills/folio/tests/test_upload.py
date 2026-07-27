#!/usr/bin/env python3
import importlib.util
import io
import pathlib
import sys
import unittest
from unittest import mock


SCRIPT = pathlib.Path(__file__).parents[1] / "scripts" / "upload.py"
SPEC = importlib.util.spec_from_file_location("folio_upload", SCRIPT)
upload = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = upload
SPEC.loader.exec_module(upload)


class FakeResponse:
    def __init__(self, status_code=201, location="/files/random.pdf", text=""):
        self.status_code = status_code
        self.headers = {"location": location} if location is not None else {}
        self.text = text


class UploadScriptTests(unittest.TestCase):
    def setUp(self):
        self.argv = [
            "upload.py",
            "--file", "/tmp/report.pdf",
            "--url", "https://example.test/uploads",
        ]
        self.args_patch = mock.patch.object(sys, "argv", self.argv)
        self.args_patch.start()
        self.addCleanup(self.args_patch.stop)

    def test_resolves_relative_and_absolute_locations(self):
        self.assertEqual(
            upload.resolve_location(
                "https://example.test/api/uploads", "/files/random.pdf"
            ),
            "https://example.test/files/random.pdf",
        )
        self.assertEqual(
            upload.resolve_location(
                "https://example.test/api/uploads", "https://cdn.test/file.pdf"
            ),
            "https://cdn.test/file.pdf",
        )

    def test_default_upload_url_is_folio_service(self):
        self.argv[:] = ["upload.py", "--file", "/tmp/report.pdf"]
        with mock.patch.object(upload.ua_generator, "generate") as generate:
            generate.return_value.text = "test-agent"
            with mock.patch.object(upload.requests, "post", return_value=FakeResponse()) as post:
                with mock.patch("builtins.open", mock.mock_open(read_data=b"pdf")):
                    with mock.patch("sys.stdout", new_callable=io.StringIO):
                        upload.main()
        self.assertEqual(post.call_args.args[0], "https://folio.weii.cloud/uploads")

    def test_upload_sends_expiry_and_private_emails(self):
        response = FakeResponse()
        with mock.patch.object(upload.ua_generator, "generate") as generate:
            generate.return_value.text = "test-agent"
            with mock.patch.object(upload.requests, "post", return_value=response) as post:
                self.argv.extend([
                    "--expire", "7d",
                    "--emails", "alice@example.com,bob@example.com",
                ])
                with mock.patch("builtins.open", mock.mock_open(read_data=b"pdf")):
                    with mock.patch("sys.stdout", new_callable=io.StringIO) as stdout:
                        upload.main()

        output_lines = stdout.getvalue().strip().splitlines()
        self.assertEqual(output_lines[0], "https://example.test/files/random.pdf")
        self.assertRegex(output_lines[1], r"^Expires: \d{4}-\d{2}-\d{2}T")
        self.assertEqual(post.call_args.kwargs["params"], {"expire": "7d"})
        self.assertEqual(
            post.call_args.kwargs["data"],
            {"authorized_emails": "alice@example.com,bob@example.com"},
        )
        self.assertFalse(post.call_args.kwargs["allow_redirects"])
        self.assertEqual(post.call_args.kwargs["timeout"], 120)

    def test_missing_location_is_failure(self):
        response = FakeResponse(location=None)
        with mock.patch.object(upload.ua_generator, "generate") as generate:
            generate.return_value.text = "test-agent"
            with mock.patch.object(upload.requests, "post", return_value=response):
                with mock.patch("builtins.open", mock.mock_open(read_data=b"pdf")):
                    with self.assertRaises(SystemExit) as error:
                        upload.main()
        self.assertEqual(error.exception.code, 1)

    def test_non_success_response_is_failure(self):
        response = FakeResponse(status_code=413, text="too large")
        with mock.patch.object(upload.ua_generator, "generate") as generate:
            generate.return_value.text = "test-agent"
            with mock.patch.object(upload.requests, "post", return_value=response):
                with mock.patch("builtins.open", mock.mock_open(read_data=b"pdf")):
                    with self.assertRaises(SystemExit) as error:
                        upload.main()
        self.assertEqual(error.exception.code, 1)

    def test_authorization_and_rate_limit_responses_are_failures(self):
        for status in (401, 403, 429):
            with self.subTest(status=status):
                response = FakeResponse(status_code=status)
                with mock.patch.object(upload.ua_generator, "generate") as generate:
                    generate.return_value.text = "test-agent"
                    with mock.patch.object(upload.requests, "post", return_value=response):
                        with mock.patch("builtins.open", mock.mock_open(read_data=b"pdf")):
                            with self.assertRaises(SystemExit) as error:
                                upload.main()
                self.assertEqual(error.exception.code, 1)

    def test_unsafe_location_is_failure(self):
        response = FakeResponse(location="javascript:alert(1)")
        with mock.patch.object(upload.ua_generator, "generate") as generate:
            generate.return_value.text = "test-agent"
            with mock.patch.object(upload.requests, "post", return_value=response):
                with mock.patch("builtins.open", mock.mock_open(read_data=b"pdf")):
                    with self.assertRaises(SystemExit) as error:
                        upload.main()
        self.assertEqual(error.exception.code, 1)

    def test_invalid_expiry_is_rejected_before_upload(self):
        with self.assertRaises(SystemExit) as error:
            self.argv.extend(["--expire", "7weeks"])
            upload.main()
        self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
