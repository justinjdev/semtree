"""Tests for semtree.summarizer module."""

import subprocess
from unittest.mock import patch, MagicMock

import pytest

from semtree.summarizer import (
    build_file_prompt,
    build_dir_prompt,
    is_oversized,
    ClaudeCLISummarizer,
)


class TestBuildFilePrompt:
    def test_includes_path_and_content(self):
        prompt = build_file_prompt("src/main.py", "print('hello')")
        assert "src/main.py" in prompt
        assert "print('hello')" in prompt


class TestBuildDirPrompt:
    def test_includes_children_with_bold_paths(self):
        children = [
            ("src/foo.py", "Does foo things."),
            ("src/bar.py", "Does bar things."),
        ]
        prompt = build_dir_prompt("src", children)
        assert "**src/foo.py**" in prompt
        assert "**src/bar.py**" in prompt
        assert "Does foo things." in prompt
        assert "Does bar things." in prompt

    def test_empty_path_uses_repository_root(self):
        prompt = build_dir_prompt("", [("README.md", "Project readme.")])
        assert "(repository root)" in prompt


class TestIsOversized:
    def test_returns_true_when_exceeds_limit(self):
        # 40_001 bytes / 4 = 10_000.25 > 10_000
        assert is_oversized(40_001, max_tokens=10_000) is True

    def test_returns_false_when_within_limit(self):
        # 40_000 bytes / 4 = 10_000 == 10_000, not greater
        assert is_oversized(40_000, max_tokens=10_000) is False


class TestClaudeCLISummarizer:
    def test_summarize_success(self):
        summarizer = ClaudeCLISummarizer()
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stdout = "  A concise summary.\n"

        with patch("semtree.summarizer.subprocess.run", return_value=mock_result) as mock_run:
            result = summarizer.summarize("some prompt")

        assert result == "A concise summary."
        mock_run.assert_called_once()

    def test_summarize_retry_then_success(self):
        summarizer = ClaudeCLISummarizer(max_retries=3)

        fail_result = MagicMock()
        fail_result.returncode = 1
        fail_result.stdout = ""
        fail_result.stderr = "temporary error"

        success_result = MagicMock()
        success_result.returncode = 0
        success_result.stdout = "Got it on retry.\n"

        with patch("semtree.summarizer.subprocess.run", side_effect=[fail_result, success_result]) as mock_run:
            with patch("semtree.summarizer.time.sleep"):
                result = summarizer.summarize("prompt")

        assert result == "Got it on retry."
        assert mock_run.call_count == 2

    def test_summarize_raises_when_claude_not_found(self):
        summarizer = ClaudeCLISummarizer()

        with patch("semtree.summarizer.subprocess.run", side_effect=FileNotFoundError):
            with pytest.raises(RuntimeError, match="claude CLI not found"):
                summarizer.summarize("prompt")

    def test_summarize_raises_after_all_retries_exhausted(self):
        summarizer = ClaudeCLISummarizer(max_retries=2)

        fail_result = MagicMock()
        fail_result.returncode = 1
        fail_result.stdout = ""
        fail_result.stderr = "persistent error"

        with patch("semtree.summarizer.subprocess.run", return_value=fail_result):
            with patch("semtree.summarizer.time.sleep"):
                with pytest.raises(RuntimeError, match="claude CLI failed"):
                    summarizer.summarize("prompt")
