// Every URL the extension opens for the user. Nothing else in the code spells one (03 §2): moving the repo changes this file and package.json.

export const REPO = "https://github.com/yaredtsy/vnoc";
export const DISCORD = "https://discord.gg/J5nfPHqyBr";
/** the GitHub issue form (`.github/ISSUE_TEMPLATE/bug_report.yml`); the fields of `reportUrl` are its ids */
export const ISSUE_FORM = `${REPO}/issues/new?template=bug_report.yml`;

/** A file of the repository as it was at a release: `v0.2.0` is the tag the release is cut from. */
export const atTag = (version: string, file: string) => `${REPO}/blob/v${version}/${file}`;

export const changelogUrl = (version: string) => atTag(version, "extension/CHANGELOG.md");
export const licenceUrl = (version: string) => atTag(version, "LICENSE");
