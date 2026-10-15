// Landing page: screenshot gallery, install tabs and copy button.
const SCREENS = [
  '01-editor', '02-travel', '03-recipe', '04-languages', '05-search', '06-palette',
  '07-history', '08-export', '09-private', '10-themes', '11-mcp', '12-spanish'
];

let currentIndex = 0;
let activeOS = 'windows';

function buildGallery() {
  const display = document.getElementById('gallery-display');
  const thumbs = document.getElementById('gallery-thumbnails');
  SCREENS.forEach((name, i) => {
    const slide = document.createElement('div');
    slide.className = 'gallery-slide' + (i === 0 ? ' active' : '');
    slide.innerHTML = `<img src="assets/screens/${name}.webp" width="1586" height="993" loading="lazy" alt="">`;
    display.insertBefore(slide, display.querySelector('.gallery-nav'));

    const thumb = document.createElement('button');
    thumb.className = 'gallery-thumb' + (i === 0 ? ' active' : '');
    thumb.innerHTML = `<img src="assets/screens/${name}-thumb.webp" width="320" height="200" loading="lazy" alt="">`;
    thumb.addEventListener('click', () => showSlide(i));
    thumbs.appendChild(thumb);
  });
  document.getElementById('next-btn').addEventListener('click', () => showSlide(currentIndex + 1));
  document.getElementById('prev-btn').addEventListener('click', () => showSlide(currentIndex - 1));
  document.addEventListener('keydown', e => {
    const rect = display.getBoundingClientRect();
    if (rect.bottom < 0 || rect.top > window.innerHeight) return;
    if (e.key === 'ArrowRight') showSlide(currentIndex + 1);
    if (e.key === 'ArrowLeft') showSlide(currentIndex - 1);
  });
}

function showSlide(index, scroll = true) {
  const slides = document.querySelectorAll('.gallery-slide');
  const thumbs = document.querySelectorAll('.gallery-thumb');
  currentIndex = (index + slides.length) % slides.length;
  slides.forEach((s, i) => s.classList.toggle('active', i === currentIndex));
  thumbs.forEach((th, i) => {
    th.classList.toggle('active', i === currentIndex);
    if (i === currentIndex && scroll) {
      const strip = th.parentElement;
      strip.scrollTo({ left: th.offsetLeft - strip.clientWidth / 2 + th.clientWidth / 2, behavior: 'smooth' });
    }
  });
  const captions = translations[currentLang].gallery;
  document.getElementById('gallery-caption').textContent = captions[currentIndex][0];
  slides.forEach((s, i) => s.querySelector('img').alt = captions[i][1]);
  thumbs.forEach((th, i) => th.querySelector('img').alt = captions[i][1]);
}

function switchTab(os) {
  activeOS = os;
  document.getElementById('tab-linux').classList.toggle('active', os === 'linux');
  document.getElementById('tab-windows').classList.toggle('active', os === 'windows');
  document.getElementById('code-linux').style.display = os === 'linux' ? 'block' : 'none';
  document.getElementById('code-windows').style.display = os === 'windows' ? 'block' : 'none';
  document.getElementById('terminal-title').textContent = os === 'linux' ? 'install.sh' : 'install.ps1';
  resetCopyButton();
}

function resetCopyButton() {
  const btn = document.getElementById('btn-copy');
  btn.classList.remove('copied');
  btn.textContent = t('btn_copy');
}

function copyCommand() {
  const text = document.getElementById(activeOS === 'linux' ? 'cmd-linux' : 'cmd-windows').textContent.trim();
  const btn = document.getElementById('btn-copy');
  navigator.clipboard.writeText(text).then(() => {
    btn.classList.add('copied');
    btn.textContent = t('btn_copied');
    setTimeout(resetCopyButton, 3000);
  }).catch(err => console.error('Copy failed:', err));
}

// Linux visitors see the Linux command first.
if (/Linux|X11/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent)) {
  document.addEventListener('DOMContentLoaded', () => switchTab('linux'));
}

buildGallery();
onLanguageChange.push(() => showSlide(currentIndex, false));
