import { describe, expect, it } from 'vitest';

import { classifyLink, linkHost } from './link';

describe('classificação dos links (mesma regra do backend)', () => {
  it('http:// é recusado antes de tudo (CA-T08-07)', () => {
    for (const text of [
      'http://example.com/mod.jar',
      'HTTP://EXAMPLE.COM/MOD.JAR',
      '  http://modrinth.com/mod/sodium ',
      'http://www.curseforge.com/minecraft/mc-mods/jei',
    ]) {
      expect(classifyLink(text), text).toBe('insecure');
    }
  });

  it('texto comum é busca', () => {
    for (const text of ['sodium', '', '  jei ', 'mod.jar', 'https']) {
      expect(classifyLink(text), text).toBeNull();
    }
  });

  it('links do Modrinth: projeto, versão e CDN', () => {
    for (const text of [
      'https://modrinth.com/mod/sodium',
      'https://www.modrinth.com/mod/sodium/versions',
      'https://modrinth.com/shader/complementary-reimagined',
      'https://modrinth.com/mod/sodium/version/mc1.21.1-0.6.0-fabric',
      'https://cdn.modrinth.com/data/AANobbMI/versions/SMxNOGZ6/sodium-fabric-0.8.13.jar',
    ]) {
      expect(classifyLink(text), text).toBe('modrinth');
    }
    expect(classifyLink('https://modrinth.com/user/alguem')).toBe('unsupported');
  });

  it('links da CurseForge seguem pela P1-10', () => {
    expect(classifyLink('https://www.curseforge.com/minecraft/mc-mods/jei')).toBe('curseforge');
    expect(classifyLink('https://www.curseforge.com/minecraft/mc-mods/jei/files/4712345')).toBe(
      'curseforge',
    );
  });

  it('link direto só para .jar e .zip, sem senha', () => {
    expect(classifyLink('https://example.com/files/Meu%20Mod%2B1.0.jar')).toBe('direct');
    expect(classifyLink('https://example.com/Faithful.ZIP?download=1')).toBe('direct');
    for (const text of [
      'https://example.com/',
      'https://example.com/page.html',
      'https://user:senha@example.com/mod.jar',
      'https://example.com/a%3Ab.jar',
      'https://example.com/con.jar',
      'https://example.com/LPT1.tar.zip',
      `https://example.com/${'ç'.repeat(99)}.jar`,
      'ftp://example.com/mod.jar',
      `https://example.com/${'a'.repeat(2048)}.jar`,
    ]) {
      expect(classifyLink(text), text).toBe('unsupported');
    }
  });

  it('servidor do link', () => {
    expect(linkHost(' https://files.example.com/a.jar ')).toBe('files.example.com');
    expect(linkHost('não é link')).toBe('');
  });
});
