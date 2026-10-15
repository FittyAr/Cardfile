// Changelog page: reads CHANGELOG.md (git-cliff format) from the main branch.
const CHANGELOG_URL = 'https://raw.githubusercontent.com/FittyAr/Cardfile/main/CHANGELOG.md';
const VISIBLE_RELEASES = 3;
let showOlder = false;
let releases = [];

function escapeHtml(text) {
  return (text || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

// Inline Markdown subset: `code`, **bold**, *italic* and [links](https://…).
function inline(text) {
  return escapeHtml(text)
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^*])\*([^*]+)\*/g, '$1<em>$2</em>')
    .replace(/\[([^\]]+)\]\((https:\/\/[^)\s]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
}

function parseChangelog(md) {
  const out = [];
  let release = null;
  let category = null;
  let last = null; // Item or intro being continued by wrapped lines.
  for (const raw of md.split('\n')) {
    const line = raw.trimEnd();
    const version = line.match(/^##\s+\[([^\]]+)\](?:\s*-\s*(.+))?$/);
    if (version) {
      release = { version: version[1], date: (version[2] || 'Unreleased').trim(), intro: [], categories: [] };
      out.push(release);
      category = null;
      last = null;
      continue;
    }
    if (!release) continue;
    const cat = line.match(/^###\s+(.+)$/);
    if (cat) {
      category = { name: cat[1].trim(), items: [] };
      release.categories.push(category);
      last = null;
      continue;
    }
    if (!line.trim()) {
      last = null;
      continue;
    }
    const item = line.match(/^\s*[-*]\s+(.+)$/);
    if (item && category) {
      category.items.push(item[1]);
      last = { list: category.items, index: category.items.length - 1 };
    } else if (last) {
      last.list[last.index] += ' ' + line.trim();
    } else if (!category) {
      release.intro.push(line.trim());
      last = { list: release.intro, index: release.intro.length - 1 };
    }
  }
  return out;
}

function categoryLabel(name) {
  const key = 'changelog_cat_' + name.toLowerCase().replace(/[^a-z]+/g, '_');
  const value = translations[currentLang]?.[key];
  return value || name;
}

function render() {
  const container = document.getElementById('changelog-container');
  container.innerHTML = '';
  releases.forEach((release, idx) => {
    const item = document.createElement('div');
    item.className = 'changelog-item' + (idx >= VISIBLE_RELEASES ? ' older-release' : '') + (showOlder ? ' show' : '');
    const cats = release.categories.filter(c => c.items.length).map(c => `
      <div class="changelog-category">
        <span class="changelog-category-title cat-${escapeHtml(c.name.toLowerCase().replace(/[^a-z]+/g, '-'))}">${escapeHtml(categoryLabel(c.name))}</span>
        <ul class="changelog-list">${c.items.map(i => `<li>${inline(i)}</li>`).join('')}</ul>
      </div>`).join('');
    const date = release.date === 'Unreleased' ? t('changelog_unreleased') : release.date;
    item.innerHTML = `
      <div class="changelog-dot"></div>
      <div class="changelog-card">
        <div class="changelog-header">
          <div class="changelog-version">${escapeHtml(release.version)}
            ${idx === 0 ? `<span class="latest-tag">${escapeHtml(t('changelog_latest'))}</span>` : ''}</div>
          <div class="changelog-date">${escapeHtml(date)}</div>
        </div>
        ${release.intro.length ? `<p class="changelog-intro">${release.intro.map(inline).join(' ')}</p>` : ''}
        <div class="changelog-body">${cats}</div>
      </div>`;
    container.appendChild(item);
  });
  document.getElementById('changelog-actions').style.display = releases.length > VISIBLE_RELEASES ? 'flex' : 'none';
  updateToggle();
}

function updateToggle() {
  document.getElementById('btn-toggle-text').textContent = t(showOlder ? 'btn_hide_older' : 'btn_show_older');
  document.getElementById('btn-toggle-icon').textContent = showOlder ? '▲' : '▼';
}

function toggleOlderReleases() {
  showOlder = !showOlder;
  document.querySelectorAll('.changelog-item.older-release').forEach(el => el.classList.toggle('show', showOlder));
  updateToggle();
}

async function loadChangelog() {
  try {
    const response = await fetch(CHANGELOG_URL);
    if (!response.ok) throw new Error('Failed to fetch CHANGELOG.md');
    releases = parseChangelog(await response.text());
    if (releases.length === 0) throw new Error('No releases parsed');
    render();
  } catch (error) {
    console.error('Error loading changelog:', error);
    const status = document.getElementById('changelog-status');
    if (status) {
      status.setAttribute('data-i18n', 'changelog_error');
      status.textContent = t('changelog_error');
    }
  }
}

onLanguageChange.push(() => { if (releases.length) render(); });
document.addEventListener('DOMContentLoaded', loadChangelog);
