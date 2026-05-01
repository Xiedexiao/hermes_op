import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const testDir = dirname(fileURLToPath(import.meta.url));
const uiRoot = resolve(testDir, '..');
const skillsPageSource = readFileSync(resolve(uiRoot, 'src/routes/SkillsPage.tsx'), 'utf8');

test('skills page exposes local marketplace history audit filters and export actions', () => {
  assert.match(
    skillsPageSource,
    /\bskillsMarketplaceListInstallHistory\b/,
    'SkillsPage should keep using skillsMarketplaceListInstallHistory',
  );
  assert.match(
    skillsPageSource,
    /marketplace_id:\s*normalizedFilters\.marketplaceId\s*\|\|\s*undefined/,
    'SkillsPage should pass the marketplace id filter to the history request',
  );
  assert.match(
    skillsPageSource,
    /skill_name:\s*normalizedFilters\.skillName\s*\|\|\s*undefined/,
    'SkillsPage should pass the skill or installed-name filter to the history request',
  );
  assert.match(
    skillsPageSource,
    /Marketplace install history/,
    'SkillsPage should keep the marketplace install history panel visible',
  );
  assert.match(
    skillsPageSource,
    /Local audit export/i,
    'SkillsPage should expose local audit export copy',
  );
  assert.match(
    skillsPageSource,
    /Copy audit JSON/i,
    'SkillsPage should expose a copy action for local audit JSON',
  );
  assert.match(
    skillsPageSource,
    /Download audit JSON/i,
    'SkillsPage should expose a download action for local audit JSON',
  );
  assert.match(
    skillsPageSource,
    /Target remote user id|target_remote_user_id/,
    'SkillsPage should include optional future remote user routing metadata in local audit JSON',
  );
  assert.match(
    skillsPageSource,
    /future remote user routing metadata only/i,
    'SkillsPage should keep the local-only boundary for remote user audit metadata visible',
  );
  assert.match(
    skillsPageSource,
    /marketplace-install-history-audit\.json/,
    'SkillsPage should export the agreed local audit filename',
  );
});
