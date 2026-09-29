import { useEffect, useRef } from 'react';
import { Link } from 'react-router-dom';
import { GlobalStyles } from '@mui/material';
import './LandingPage.css';

// Google G logo SVG
function GoogleIcon({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
      <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" />
      <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" />
      <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" />
      <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" />
    </svg>
  );
}

// Line Icons
function IconLock({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
      <path d="M7 11V7a5 5 0 0 1 10 0v4" />
    </svg>
  );
}
function IconBell({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
      <path d="M13.73 21a2 2 0 0 1-3.46 0" />
    </svg>
  );
}
function IconGrid({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="3" width="7" height="7" />
      <rect x="14" y="3" width="7" height="7" />
      <rect x="14" y="14" width="7" height="7" />
      <rect x="3" y="14" width="7" height="7" />
    </svg>
  );
}
function IconCircle({ size = 16, filled = false }: { size?: number; filled?: boolean }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
    </svg>
  );
}
function IconSettings({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M12 1v6m0 6v6m4.22-10.22l4.24-4.24M6.34 17.66l-4.24 4.24M23 12h-6m-6 0H1m20.24 4.24l-4.24-4.24M6.34 6.34L2.1 2.1" />
    </svg>
  );
}
function IconChevronDown({ size = 12 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="6 9 12 15 18 9" />
    </svg>
  );
}
function IconChevronRight({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="9 18 15 12 9 6" />
    </svg>
  );
}
function IconTrendingUp({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="23 6 13.5 15.5 8.5 10.5 1 18" />
      <polyline points="17 6 23 6 23 12" />
    </svg>
  );
}
function IconDollar({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="12" y1="1" x2="12" y2="23" />
      <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6" />
    </svg>
  );
}
function IconRefresh({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="23 4 23 10 17 10" />
      <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
    </svg>
  );
}
function IconCompass({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
      <polygon points="16.24 7.76 14.12 14.12 7.76 16.24 9.88 9.88 16.24 7.76" />
    </svg>
  );
}

export default function LandingPage() {
  const navRef = useRef<HTMLElement>(null);
  const mockupLabelRef = useRef<HTMLDivElement>(null);
  const browserWrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const nav = navRef.current;
    const hero = document.querySelector('.hero');
    const heroHeight = hero?.getBoundingClientRect().height || window.innerHeight;

    const handleScroll = () => {
      if (nav) {
        // Switch to cream background after scrolling past 70% of hero
        nav.classList.toggle('scrolled', window.scrollY > heroHeight * 0.7);
      }
    };
    window.addEventListener('scroll', handleScroll, { passive: true });
    handleScroll(); // Check initial position

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((e) => {
          if (e.isIntersecting) {
            e.target.classList.add('visible');
            observer.unobserve(e.target);
          }
        });
      },
      { threshold: 0.06, rootMargin: '0px 0px -32px 0px' }
    );

    document.querySelectorAll('.reveal').forEach((el) => observer.observe(el));
    if (mockupLabelRef.current) observer.observe(mockupLabelRef.current);
    if (browserWrapRef.current) observer.observe(browserWrapRef.current);

    return () => {
      window.removeEventListener('scroll', handleScroll);
      observer.disconnect();
    };
  }, []);

  return (
    <div className="landing-page-root">
      <GlobalStyles styles={{ body: { overflow: 'auto !important' } }} />

      {/* NAV */}
      <nav className="nav" ref={navRef}>
        <Link to="/" className="logo">Sudo<em>query</em></Link>
        <ul className="nav-links">
          <li><a href="#features">Features</a></li>
          <li><a href="#how">How it works</a></li>
          <li><a href="/docs">Docs</a></li>
        </ul>
        <div className="nav-r">
          <Link to="/login" className="nav-si">Sign in</Link>
        </div>
      </nav>

      {/* HERO */}
      <section className="hero">
        <div className="hero-bg"></div>
        <div className="hero-vignette"></div>
        <div className="hero-content">
          <h1>Ask what<br/>your data<br/><em>knows.</em></h1>
          <p className="hero-deck">Skip the dashboard. Ask your product data plain questions and get instant charts, trends, and explanations. No analyst. No waiting.</p>
          <div className="hero-cta">
            <Link to="/login" className="btn-hero"><GoogleIcon size={20} /> Continue with Google</Link>
          </div>
        </div>
      </section>

      {/* DESKTOP MOCKUP */}
      <section className="mockup-section" id="mockup-section">
        <div className="browser-wrap" ref={browserWrapRef}>
          <div className="browser-bar">
            <div className="bdots"><div className="bdot bdot-r"></div><div className="bdot bdot-y"></div><div className="bdot bdot-g"></div></div>
            <div className="browser-url">
              <span className="url-lock"><IconLock size={12} /></span>
              <span className="url-text"><span>app.sudoquery.so/</span>acme-corp/growth/chat</span>
            </div>
            <div className="browser-actions">
              <span className="bact">⟳</span>
            </div>
          </div>
          <div className="app-shell">
            <div className="app-topbar">
              <div className="sw"><span className="swo">Acme Corp</span><span className="sws">/</span><span className="swp">Growth</span><span className="swc"><IconChevronDown size={12} /></span></div>
              <div className="tsp"></div>
              <div className="tapx">
                <div className="ticon"><IconBell size={16} /></div>
                <div className="ticon"><IconGrid size={16} /></div>
                <div className="uav">AR</div>
              </div>
            </div>
            <div className="app-body">
              <div className="snav">
                <div className="sni sna"><IconCircle size={16} filled /></div>
                <div className="sni"><IconGrid size={16} /></div>
                <div className="snsp"></div>
                <div className="sni"><IconSettings size={16} /></div>
              </div>
              <div className="app-main">
                <div className="app-bc">
                  <span>Acme Corp</span><span className="bcsep"><IconChevronRight size={10} /></span>
                  <span>Growth</span><span className="bcsep"><IconChevronRight size={10} /></span>
                  <span className="bcact">Chat</span>
                </div>
                <div className="chat-body">
                  <div className="ewel">Create dashboards or explore your data</div>
                  <div>
                    <div className="clbl">Growth</div>
                    <div className="chips">
                      <div className="qc">How are signups trending this month? <span className="qa">→</span></div>
                      <div className="qc">What's our best acquisition channel? <span className="qa">→</span></div>
                    </div>
                  </div>
                  <div>
                    <div className="clbl">Revenue</div>
                    <div className="chips">
                      <div className="qc">Show me MRR by plan <span className="qa">→</span></div>
                    </div>
                  </div>
                  <div className="tsep">Today</div>
                  <div className="mu"><div className="bu">What drove the signup spike on July 11?</div></div>
                  <div className="mai">
                    <div className="aiav">q</div>
                    <div className="bai">
                      The July 11 spike followed your product launch — <strong>+340%</strong> vs the prior 7-day average. Organic search led the surge.
                      <div className="mc">
                        <div className="mch">
                          <span className="mcl">Daily signups · July 2024</span>
                          <div><span className="mcv">1,847</span><span className="mcd">+23%</span></div>
                        </div>
                        <div className="bars">
                          <div className="bar"></div><div className="bar"></div><div className="bar"></div><div className="bar"></div>
                          <div className="bar hi"></div><div className="bar"></div><div className="bar"></div>
                          <div className="bar hi"></div><div className="bar hi"></div><div className="bar hi"></div>
                        </div>
                        <div className="xax">
                          <span className="xt">Jul 1</span><span className="xt">Jul 8</span>
                          <span className="xt">Jul 15</span><span className="xt">Jul 22</span><span className="xt">Jul 31</span>
                        </div>
                      </div>
                    </div>
                  </div>
                  <div className="mu"><div className="bu">Which channel converted best? <span className="cur"></span></div></div>
                  <div className="mai">
                    <div className="aiav">q</div>
                    <div className="bai"><div className="tdots"><div className="td"></div><div className="td"></div><div className="td"></div></div></div>
                  </div>
                </div>
                <div className="cib">
                  <div className="cif">Ask a follow-up question…</div>
                  <div className="cis">↑</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* HOW */}
      <section className="howsec reveal" id="how">
        <div className="howgrid">
          <div className="hwl">
            <div className="seclbl"><div className="secrl"></div><span className="sectl">How it works</span></div>
            <h2>Three steps to<br/><em>your first insight.</em></h2>
            <p>One SDK install. No BI tool to learn. Just ask questions in plain English.</p>
            <div className="steps">
              <div className="step"><div className="stepn">1</div><div><div className="stept">Install the SDK</div><div className="stepd">Add our lightweight SDK to your app with one line of code. Works with any backend — Node, Python, Go, Ruby, and more.</div></div></div>
              <div className="step"><div className="stepn">2</div><div><div className="stept">Send events with your API key</div><div className="stepd">Start tracking events using your unique API key. No schema setup required — just send what matters to your product.</div></div></div>
              <div className="step"><div className="stepn">3</div><div><div className="stept">Ask questions, get insights</div><div className="stepd">Open Sudoquery and start asking questions in plain English. Get charts, trends, and explanations instantly.</div></div></div>
            </div>
          </div>
          <div>
            <div className="seclbl"><div className="secrl"></div><span className="sectl">Ask anything</span></div>
            <div className="ccards">
              <div className="ccard"><div className="cchead"><div className="ccico"><IconTrendingUp size={16} /></div><span className="ccname">Growth & Signups</span></div><div className="ccq">"How are signups trending vs last month, by channel?"</div></div>
              <div className="ccard"><div className="cchead"><div className="ccico"><IconDollar size={16} /></div><span className="ccname">Revenue & MRR</span></div><div className="ccq">"Show me MRR growth over 6 months, segmented by plan."</div></div>
              <div className="ccard"><div className="cchead"><div className="ccico"><IconRefresh size={16} /></div><span className="ccname">Retention & Churn</span></div><div className="ccq">"What's our 30-day retention for users who signed up in April?"</div></div>
              <div className="ccard"><div className="cchead"><div className="ccico"><IconCompass size={16} /></div><span className="ccname">Behaviour & Funnels</span></div><div className="ccq">"Where do users drop off in our onboarding funnel?"</div></div>
            </div>
          </div>
        </div>
      </section>

      {/* CTA BANNER */}
      <section className="ctabanner reveal">
        <div className="banleft"><h2>Your data has been<br/>waiting to <em>talk.</em></h2><p>Set up in 2 minutes. No SQL required.</p></div>
        <div className="banright">
          <Link to="/login" className="btnban"><GoogleIcon size={20} /> Try Sudoquery with Google</Link>
          <span className="bann">No credit card · get started in minutes</span>
        </div>
      </section>

      {/* FOOTER */}
      <footer>
        <div><div className="flogo">Sudo<em>query</em></div><p className="ftag">AI analytics for teams who want answers, not dashboards.</p></div>
        <div><div className="fcollbl">Product</div><ul className="flinks"><li><a href="#">How it works</a></li><li><a href="#">Changelog</a></li><li><a href="#">Roadmap</a></li></ul></div>
        <div><div className="fcollbl">Company</div><ul className="flinks"><li><a href="#">About</a></li><li><a href="#">Blog</a></li><li><a href="#">Careers</a></li><li><a href="#">Contact</a></li></ul></div>
        <div><div className="fcollbl">Legal</div><ul className="flinks"><li><a href="#">Privacy policy</a></li><li><a href="#">Terms of service</a></li><li><a href="#">Security</a></li><li><a href="#">GDPR</a></li></ul></div>
      </footer>
      <div className="fbot"><span className="fleg">© 2024 Sudoquery Inc. All rights reserved.</span><span className="fleg"><a href="#">Privacy</a> · <a href="#">Terms</a></span></div>

    </div>
  );
}
