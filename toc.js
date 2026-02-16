// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded affix "><a href="index.html">Introduction</a></li><li class="chapter-item expanded "><a href="installation/index.html"><strong aria-hidden="true">1.</strong> Installation</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="installation/windows.html"><strong aria-hidden="true">1.1.</strong> Windows</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="installation/windows-msvc.html"><strong aria-hidden="true">1.1.1.</strong> MSVC prerequisites</a></li></ol></li><li class="chapter-item expanded "><a href="installation/other.html"><strong aria-hidden="true">1.2.</strong> Other installation methods</a></li><li class="chapter-item expanded "><a href="installation/already-installed-rust.html"><strong aria-hidden="true">1.3.</strong> Already installed Rust?</a></li></ol></li><li class="chapter-item expanded "><a href="concepts/index.html"><strong aria-hidden="true">2.</strong> Concepts</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="concepts/channels.html"><strong aria-hidden="true">2.1.</strong> Channels</a></li><li class="chapter-item expanded "><a href="concepts/toolchains.html"><strong aria-hidden="true">2.2.</strong> Toolchains</a></li><li class="chapter-item expanded "><a href="concepts/components.html"><strong aria-hidden="true">2.3.</strong> Components</a></li><li class="chapter-item expanded "><a href="concepts/profiles.html"><strong aria-hidden="true">2.4.</strong> Profiles</a></li><li class="chapter-item expanded "><a href="concepts/proxies.html"><strong aria-hidden="true">2.5.</strong> Proxies</a></li></ol></li><li class="chapter-item expanded "><a href="basics.html"><strong aria-hidden="true">3.</strong> Basic usage</a></li><li class="chapter-item expanded "><a href="overrides.html"><strong aria-hidden="true">4.</strong> Overrides</a></li><li class="chapter-item expanded "><a href="cross-compilation.html"><strong aria-hidden="true">5.</strong> Cross-compilation</a></li><li class="chapter-item expanded "><a href="environment-variables.html"><strong aria-hidden="true">6.</strong> Environment variables</a></li><li class="chapter-item expanded "><a href="configuration.html"><strong aria-hidden="true">7.</strong> Configuration</a></li><li class="chapter-item expanded "><a href="network-proxies.html"><strong aria-hidden="true">8.</strong> Network proxies</a></li><li class="chapter-item expanded "><a href="examples.html"><strong aria-hidden="true">9.</strong> Examples</a></li><li class="chapter-item expanded "><a href="security.html"><strong aria-hidden="true">10.</strong> Security</a></li><li class="chapter-item expanded "><a href="faq.html"><strong aria-hidden="true">11.</strong> FAQ</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString();
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
