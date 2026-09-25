"""Behavioral tests for rendered source HTML to World Anvil BBCode."""

import unittest

from tools.cobalt_migration.worldanvil_html import (
    UnsupportedContent,
    convert_html,
    literal_text,
)

SOURCE = "https://cobalt-company.wikidot.com/character:abigael"


class RenderedContentTests(unittest.TestCase):
    def convert(self, body, image_ref=None):
        return convert_html(
            f'<header>Chrome</header><div id="page-content">{body}</div><footer>End</footer>',
            SOURCE,
            image_ref=image_ref,
        )

    def test_preserves_text_entities_inline_formatting_and_paragraphs(self):
        result = self.convert(
            "<h2>Tea &amp; <em>honey</em></h2><p>A <strong>bold</strong> &lt;hero&gt; "
            "<span>with <u>care</u> and <s>time</s></span>.</p><p>Next<br>line</p>"
        )
        self.assertEqual(
            result,
            "[h2]Tea & [i]honey[/i][/h2]\n"
            "[p]A [b]bold[/b] <hero> with [u]care[/u] and [s]time[/s].[/p]\n"
            "[p]Next[br]line[/p]",
        )

    def test_lists_quote_rule_and_code(self):
        result = self.convert(
            "<blockquote><p>Words</p></blockquote><hr><ul><li>One</li><li>Two "
            "<code>x &lt; y</code></li></ul><ol><li>Three</li></ol>"
            "<pre>  spaced\n  lines</pre>"
        )
        self.assertEqual(
            result,
            "[quote][p]Words[/p][/quote]\n[hr]\n"
            "[ul][li]One[/li][li]Two [code]x < y[/code][/li][/ul]\n"
            "[ol][li]Three[/li][/ol]\n[code]  spaced\n  lines[/code]",
        )

    def test_void_images_links_and_nested_tables_preserve_following_content(self):
        result = self.convert(
            "<table><tr><th>Role</th><td><table><tr><td>Scout</td></tr></table>"
            '<img src="/local--files/character:abigael/icon.jpg" /></td></tr></table>'
            '<p><a href="../roster#people">Roster &amp; friends</a> after image '
            '<img src="https://example.org/second.png"><br>End</p>'
        )
        self.assertEqual(
            result,
            "[table][tr][th]Role[/th][td][table][tr][td]Scout[/td][/tr][/table]"
            "[img:https://cobalt-company.wikidot.com/local--files/character:abigael/icon.jpg|none]"
            "[/td][/tr][/table]\n"
            "[p][url:https://cobalt-company.wikidot.com/roster#people]Roster & friends[/url] "
            "after image [img:https://example.org/second.png|none][br]End[/p]",
        )

    def test_image_callback_maps_only_positive_ids_and_preserves_other_source_urls(
        self,
    ):
        def resolve(url):
            return 6815014 if url.endswith("first.jpg") else None

        self.assertEqual(
            self.convert('<img src="/first.jpg"><img src="/second.jpg">', resolve),
            "[img:6815014|none]"
            "[img:https://cobalt-company.wikidot.com/second.jpg|none]",
        )

    def test_missing_duplicate_and_nested_content_wrappers_are_rejected(self):
        for html in (
            '<div id="other">No page</div>',
            '<div id="page-content">A</div><div id="page-content">B</div>',
            '<div id="page-content">A<div id="page-content">B</div></div>',
        ):
            with self.subTest(html=html), self.assertRaises(UnsupportedContent):
                convert_html(html, SOURCE)

    def test_unsupported_content_is_never_silently_dropped(self):
        for body in (
            "<p>Before<custom-widget>lost</custom-widget>After</p>",
            '<script>alert("ignored")</script>',
            "<style>.x { display:none }</style>",
            '<iframe src="/frame"></iframe>',
            '<form><input value="Private"></form>',
            '<div style="display:none">Secret</div>',
            "<span hidden>Hidden</span>",
            '<td colspan="2">Wide</td>',
            '<td rowspan="2">Tall</td>',
            "<p>Literal [/noparse] collision</p>",
            '<a href="javascript:alert(1)">Unsafe</a>',
            '<img src="data:image/png;base64,AA">',
        ):
            with self.subTest(body=body), self.assertRaises(UnsupportedContent):
                self.convert(body)

    def test_literal_brackets_are_escaped_only_in_source_text(self):
        self.assertEqual(
            literal_text("[CW: Suicide]"), "[noparse][CW: Suicide][/noparse]"
        )
        self.assertEqual(literal_text("A [b] B"), "[noparse]A [b] B[/noparse]")
        self.assertEqual(literal_text("Unmarked"), "Unmarked")
        self.assertEqual(
            self.convert("<p>[CW: Suicide] <strong>Reader</strong> [b] text</p>"),
            "[p][noparse][CW: Suicide] [/noparse][b]Reader[/b][noparse] [b] text[/noparse][/p]",
        )
        with self.assertRaises(UnsupportedContent):
            literal_text("collision [/noparse] here")

    def test_collapsible_preserves_label_nested_content_and_following_text(self):
        body = """<p>Before</p>
        <div class="collapsible-block">
          <div class="collapsible-block-folded"><a class="collapsible-block-link" href="javascript:;">+&nbsp;Relationship Spoilers</a></div>
          <div class="collapsible-block-unfolded" style="display:none">
            <div class="collapsible-block-unfolded-link"><a class="collapsible-block-link" href="javascript:;">-&nbsp;Hide Content</a></div>
            <div class="collapsible-block-content"><p>[CW: Suicide] <img src="/portrait.jpg"></p><table><tr><td><strong>Details</strong></td></tr></table></div>
          </div>
        </div><p>After</p>"""
        self.assertEqual(
            self.convert(body),
            "[p]Before[/p]\n[spoiler=+ Relationship Spoilers][p][noparse][CW: Suicide] [/noparse]"
            "[img:https://cobalt-company.wikidot.com/portrait.jpg|none][/p]\n"
            "[table][tr][td][b]Details[/b][/td][/tr][/table][/spoiler]\n[p]After[/p]",
        )

    def test_nested_collapsibles_keep_both_labels_and_bodies(self):
        body = """<div class="collapsible-block"><div class="collapsible-block-folded"><a class="collapsible-block-link" href="javascript:;">Outer</a></div><div class="collapsible-block-unfolded" style="display:none"><div class="collapsible-block-unfolded-link"><a class="collapsible-block-link" href="javascript:;">Hide</a></div><div class="collapsible-block-content"><div class="collapsible-block"><div class="collapsible-block-folded"><a class="collapsible-block-link" href="javascript:;">Inner</a></div><div class="collapsible-block-unfolded" style="display:none"><div class="collapsible-block-unfolded-link"><a class="collapsible-block-link" href="javascript:;">Hide</a></div><div class="collapsible-block-content"><p>Secret</p></div></div></div></div></div></div>"""
        self.assertEqual(
            self.convert(body),
            "[spoiler=Outer][spoiler=Inner][p]Secret[/p][/spoiler][/spoiler]",
        )

    def test_malformed_collapsibles_and_actionable_controls_block(self):
        for body in (
            '<div class="collapsible-block"><p>Unrecognized</p></div>',
            '<div class="collapsible-block"><div class="collapsible-block-folded"><a href="javascript:;">Show</a></div><div class="collapsible-block-unfolded" style="display:none"><div class="collapsible-block-unfolded-link"><a class="collapsible-block-link" href="javascript:;">Hide</a></div><div class="collapsible-block-content"><p>Secret</p></div></div></div>',
            '<a href="javascript:;" onclick="evil()">Click</a>',
            '<a href="javascript:alert(1)">Unsafe</a>',
            '<a href="javascript:;" onmouseover="evil()">Unsafe</a>',
            '<a href="javascript:;" onclick="evil()"><img src="/icon.png"></a>',
            '<div class="collapsible-block"><div class="collapsible-block-folded"><a class="collapsible-block-link" href="javascript:;">[bad]</a></div><div class="collapsible-block-unfolded" style="display:none"><div class="collapsible-block-unfolded-link"><a class="collapsible-block-link" href="javascript:;">Hide</a></div><div class="collapsible-block-content">Content</div></div></div>',
            '<iframe src="https://www.youtube.com/embed/video"></iframe>',
        ):
            with self.subTest(body=body), self.assertRaises(UnsupportedContent):
                self.convert(body)

    def test_inert_javascript_anchor_keeps_children_without_navigation(self):
        self.assertEqual(
            self.convert(
                '<p>Before <a href="javascript:;"><img src="/icon.png"></a> '
                '<a href="javascript:;">Tab <strong>name</strong></a> After</p>'
            ),
            "[p]Before [img:https://cobalt-company.wikidot.com/icon.png|none] "
            "Tab [b]name[/b] After[/p]",
        )

    def test_observed_yui_tabs_become_all_static_labeled_sections(self):
        tabs = """
        <script type="text/javascript" src="https://d3g0gp89917ko0.cloudfront.net/v--0c0da3649c4f/common--javascript/yahooui/tabview-min.js"></script>
        <div id="wiki-tabview-f3038cdf74d3cd3321fca1504304a7cb" class="yui-navset">
          <ul class="yui-nav">
            <li class="selected"><a href="javascript:;"><em>Stories</em></a></li>
            <li><a href="javascript:;"><em>Logs</em></a></li>
          </ul>
          <div class="yui-content">
            <div id="wiki-tab-0-0"><div class="list-pages-box"><table><tr><td><a href="/writing:first">First</a></td></tr></table></div></div>
            <div id="wiki-tab-0-1" style="display:none"><div class="list-pages-box"><table><tr><td>Second<img src="/second.jpg" /></td></tr></table></div></div>
          </div>
        </div>
        <script type="text/javascript">//<![CDATA[
        OZONE.dom.onDomReady(function(){
          var tabViewf3038cdf74d3cd3321fca1504304a7cb = new YAHOO.widget.TabView('wiki-tabview-f3038cdf74d3cd3321fca1504304a7cb');
        }, "dummy-ondomready-block");
        //]]></script>"""
        result = self.convert("<h1>Writings</h1>" + tabs + "<p>After</p>")
        self.assertEqual(
            result,
            "[h1]Writings[/h1]\n[h2]Stories[/h2]\n"
            "[table][tr][td][url:https://cobalt-company.wikidot.com/writing:first]First[/url]"
            "[/td][/tr][/table]\n[h2]Logs[/h2]\n"
            "[table][tr][td]Second[img:https://cobalt-company.wikidot.com/second.jpg|none]"
            "[/td][/tr][/table]\n[p]After[/p]",
        )

    def test_malformed_tab_metadata_or_other_scripts_still_block(self):
        for body in (
            '<div class="yui-navset"><ul class="yui-nav"><li>Unmapped</li></ul></div>',
            '<script type="text/javascript">OZONE.dom.onDomReady(function(){evil()})</script>',
            '<script src="https://evil.example/tabview-min.js"></script>',
        ):
            with self.subTest(body=body), self.assertRaises(UnsupportedContent):
                self.convert(body)

    def test_bad_image_callback_references_block(self):
        for reference in (0, True, -2, "6815014", "https://other.example/image.jpg"):
            with (
                self.subTest(reference=reference),
                self.assertRaises(UnsupportedContent),
            ):
                self.convert('<img src="/first.jpg">', lambda _url, ref=reference: ref)

    def test_missing_link_or_image_source_blocks(self):
        for body in ("<a>label</a>", "<img>", '<a href="/bad]url">label</a>'):
            with self.subTest(body=body), self.assertRaises(UnsupportedContent):
                self.convert(body)


if __name__ == "__main__":
    unittest.main()
