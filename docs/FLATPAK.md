# Flatpak experimental e preparação para o Flathub

O projeto oferece um **Flatpak experimental**, ainda sem publicação no Flathub. A [variante alpha.4 com previews](releases/v0.1.0-alpha.4-flatpak-dbus6.md) inclui **Poppler, LibreOffice e WeasyPrint**, com fontes correspondentes e instruções de atualização próprias na [release do GitHub](https://github.com/janderpt/linux-system-explorer/releases/tag/v0.1.0-alpha.4). O bundle original sem conversores permanece inalterado como histórico. Veja [conversores e reprodução](FLATPAK-PREVIEW.md). A branch é `experimental`, com ID `io.github.janderpt.linux-system-explorer`, e a aplicação usa Freedesktop Platform **26.08**. A instalação nativa continua sendo a referência para as integrações do sistema ainda pendentes.

A variante recomendada usa **zbus 6.2.1 com um backport verificável de autenticação**, removendo os três avisos de manutenção D-Bus e mantendo o proxy 0.1.5. O fonte modificado, a licença MIT, a origem e o patch acompanham o pacote e o arquivo de fontes do aplicativo. Veja [proveniência, manutenção e testes](DBUS-COMPATIBILITY.md).

## Instalar e remover o protótipo

A variante recomendada **flatpak-previews-dbus6** usa zbus 6.2.1 com um patch local de autenticação sequencial no Flatpak, e funciona também com `xdg-dbus-proxy` **0.1.5** fornecido pelo Ubuntu 24.04. Foi validada com proxies **0.1.5 e 0.1.9**, sem compilar ou substituir manualmente o proxy da distribuição. As variantes alpha.4 anteriores continuam disponíveis com seus requisitos e checksums originais; baixe o arquivo com **previews-dbus** para obter esta correção.

Use Flatpak e os componentes D-Bus mantidos pela distribuição, com as atualizações normais do sistema. O runtime não atualiza o proxy do host. Veja [compatibilidade e testes D-Bus](DBUS-COMPATIBILITY.md).

Com esses componentes instalados pela sua distribuição, adicione o repositório de runtimes e instale o arquivo recebido:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user ./linux-system-explorer-0.1.0-alpha.4-flatpak-previews-dbus6-x86_64.flatpak
flatpak run --branch=experimental io.github.janderpt.linux-system-explorer
```

O repositório fornece o runtime; o aplicativo acima vem do arquivo local, **não da loja**. O bundle não inclui o runtime e precisa de rede na primeira instalação. Na release, use `FLATPAK-DBUS6-SHA256SUMS` e `FLATPAK-DBUS6-BUILD-INFO.json`; builds locais usam `SHA256SUMS` e `FLATPAK-BUILD-INFO.json`. Esses relatórios identificam fonte, binário, runtime, permissões e recursos detectados. Este protótipo não tem assinatura GPG nem atualização automática por um repositório próprio.

Para atualizar o protótipo anterior (alpha.4 com ou sem conversores, ou alpha.2), feche suas janelas e instale o novo bundle **sem desinstalar a versão anterior**:

```sh
flatpak install --user ./linux-system-explorer-0.1.0-alpha.4-flatpak-previews-dbus6-x86_64.flatpak
flatpak info --user io.github.janderpt.linux-system-explorer//experimental
```

A origem e a branch `experimental` permanecem as mesmas, e as preferências do sandbox são preservadas. A alpha.3 foi distribuída como pacote nativo. O teste da variante atual verifica diretamente a atualização da alpha.4 anterior com zbus 5.19; o pacote anterior verificou alpha.2 → alpha.4. Não use `--delete-data` ao atualizar.

Para verificar os anexos publicados, execute `sha256sum --check FLATPAK-DBUS6-SHA256SUMS`; dentro da saída local de build, use `sha256sum --check SHA256SUMS`. O manifesto de checksums é separado dos pacotes nativos.

Para remover somente este protótipo:

```sh
flatpak uninstall --user io.github.janderpt.linux-system-explorer//experimental
```

As preferências ficam em `~/.var/app/io.github.janderpt.linux-system-explorer/`, separadas da instalação nativa; não copie nem sobrescreva as configurações nativas automaticamente. Nenhum desses comandos altera o explorador padrão ou registra FileManager1. Execute explicitamente o comando Flatpak enquanto coexistirem as duas instalações.

## Recursos e diferenças do sandbox

| Área | Protótipo | Antes de distribuição na loja |
| --- | --- | --- |
| Pastas, abas, dois painéis, layouts e operações | Mesma implementação Rust; acesso aos arquivos autorizado pelo Flatpak | Exercitar arquivos descartáveis em KDE/GNOME, Wayland/X11 e mídias externas |
| Imagens, SVG e texto | Decodificação incorporada, miniaturas e edição de imagens estáticas gerando novas cópias | Conferir limites e imagens grandes em máquinas limpas |
| Áudio e vídeo | GStreamer do runtime e extensão de codecs; sem dependência do player do host | Validar codecs e áudio na sessão real |
| Compactados | Preview com Python e libarchive do runtime; extração pelo gerenciador do host indisponível | Conferir ZIP, TAR, 7z e RAR; projetar integração de extração apropriada ao sandbox |
| PDF, Office e HTML/Markdown formatado | Build atual inclui Poppler, LibreOffice e WeasyPrint; previews, páginas e miniaturas usam motores do pacote | Conferir a sessão real e manter versões/licenças/fontes; o primeiro bundle alpha.4 sem conversores continua como histórico |
| Imprimir | Portal Print; conversores e `pdfunite` incluídos no build atual preparam outros tipos e seleções | Validar o diálogo de impressão nos backends KDE/GNOME/GTK |
| Abrir e Abrir com | OpenFile do portal, passando descritor do arquivo; o seletor e as associações ficam com o desktop | Conferir o backend de portal KDE/GNOME e cancelamento |
| Associação padrão de tipos de arquivo | A escolha fica com o portal; não há gravação direta no registro MIME do host | Avaliar o fluxo oferecido por cada backend |
| Aplicativos do menu do host | Ainda indisponível; a tela explica a limitação, sem apresentar o menu interno do runtime | Projetar descoberta e lançamento apropriados ao sandbox |
| Terminal integrado | Shell e programas disponíveis **dentro do Flatpak**; identificado como Terminal (Flatpak) | Não apresentar como terminal administrativo/do host |
| Compartilhamentos Samba locais e novas conexões remotas | Gerenciamento local oculto; novas conexões informam que a integração ainda não está disponível | Projetar integração apropriada; não adicionar execução irrestrita no host |
| Pastas de rede já montadas | Podem ser acessadas pelos caminhos expostos, incluindo GVfs | Validar permissões e autenticação na sessão real |
| Lixeira | Dados XDG próprios do sandbox; não pressupõe a mesma lista da Lixeira nativa | Validar envio, restauração, purga e volumes; definir a integração com a Lixeira do desktop |
| Mostrar na pasta e reutilização pelo sistema | O protótipo não assume o papel de FileManager1 | Projetar ativação D-Bus/exportação de serviços e testar sem conflitar com o gerenciador atual |

O protótipo pede leitura/escrita no filesystem `host` porque é um gerenciador de arquivos, incluindo pastas fora de Home; caminhos reservados pelo Flatpak continuam pertencendo ao runtime. Também pede Wayland/X11, OpenGL, áudio, IPC, GVfs e apenas o namespace privado de janelas do aplicativo no D-Bus. **Não** pede rede, barramento de sessão/sistema irrestrito, `org.freedesktop.Flatpak` ou execução de comandos no host. A amplitude do filesystem ainda precisa de justificativa e revisão humana para a loja. Veja as [permissões do Flatpak](https://docs.flatpak.org/en/latest/sandbox-permissions.html).

## Reproduzir o pacote local

Requer Python 3.12+, Rust, `dpkg-deb`, ferramentas de validação de metadados, Flatpak e o proxy D-Bus fornecido pela distribuição. O script reutiliza o staging nativo com licenças/fontes MPL, importa o payload verificado dos conversores e testa os programas sob o runtime antes de exportar. O LibreOffice é reempacotado de binários upstream; isto não representa uma compilação completa de fontes para a loja.

```sh
flatpak install --user flathub org.freedesktop.Platform//26.08 org.freedesktop.Sdk//26.08
python3 scripts/flatpak-converters.py fetch
python3 scripts/flatpak-converters.py build
cargo build --release --locked
python3 scripts/flatpak.py stage
python3 scripts/flatpak.py build
```

Saída: `dist/flatpak-experimental/`, incluindo o bundle e o anexo `FLATPAK-PREVIEW-SOURCES.tar`. Distribua esse anexo no mesmo local do bundle; as licenças e fontes correspondentes estão descritas em [FLATPAK-PREVIEW.md](FLATPAK-PREVIEW.md). Os destinos devem estar vazios; não reutilize staging de outra revisão. `--binary`, `--destdir`, `--stage`, `--converters`, `--work-dir` e `--output` permitem separar os ambientes. O script não instala o aplicativo no host, muda padrões, cria tags ou publica arquivos. A ferramenta `flatpak-builder` não é necessária para esse caminho por comandos `flatpak build-*`.

O registro identifica alterações não commitadas. Para distribuir builds de teste rastreáveis, faça o staging de uma revisão limpa. O teste de ABI recusa bibliotecas não encontradas ou uma versão diferente do executável; isto não substitui testes funcionais dos codecs/portais.

## Testar instalação e atualização

O teste automatizado `scripts/smoke-flatpak.py` exige um container descartável com Flatpak, runtime, proxy da distribuição, D-Bus isolado e Xvfb. Ele instala a alpha.4 do zero, verifica a versão e a abertura gráfica, remove o app, instala o bundle anterior e atualiza **no mesmo ref**, conferindo o commit e a preservação exata das preferências. Também verifica a remoção e que os padrões MIME e as configurações nativas não foram alterados. Exemplo dentro desse ambiente, com os caminhos adaptados aos volumes montados:

```sh
mkdir -p /run/dbus /run/user/0
chmod 700 /run/user/0
export XDG_RUNTIME_DIR=/run/user/0
dbus-daemon --system --fork
dbus-run-session -- xvfb-run -a python3 scripts/smoke-flatpak.py \
  /pacotes/linux-system-explorer-0.1.0-alpha.4-flatpak-experimental-x86_64.flatpak \
  --previous /anterior/linux-system-explorer-0.1.0-alpha.2-flatpak-experimental-x86_64.flatpak \
  --report /pacotes/FLATPAK-VALIDATION.json
```

O relatório identifica a versão realmente atualizada, o commit OSTree e as capacidades presentes. Testes com Xvfb/software OpenGL validam o sandbox e o ciclo de instalação; a sessão real KDE/GNOME e Wayland continua sendo uma validação separada. O relatório e suas instruções podem ser incluídos no `SHA256SUMS` do bundle após o teste.

`--test-binary /caminho/lse-tests --fixtures /caminho/fixtures` executa grupos de testes Rust dentro do app instalado: transferências, terminal, toasts, teclado, filtro e painéis, além de miniaturas, compactados, Lixeira e preferências privadas. Com conversores incluídos, exige também testes de PDF, famílias Office, HTML/Markdown, miniaturas e preparação de impressão, sem enviar documentos à impressora. Use essa opção na validação do novo bundle. O executável de testes e as fixtures sintéticas são montados somente para leitura; não são incluídos no bundle distribuído.

## Ferramentas em container (opcional)

O [Dockerfile de ferramentas](../packaging/Dockerfile.flatpak-tools) permite exportar sem instalar Flatpak no host. Faça primeiro o staging nativo acima. O container é descartável e recebe somente a pasta do projeto como leitura; `--privileged` é necessário para o bubblewrap aninhado, apenas nesse ambiente de build/teste.

```sh
docker build -t lse-flatpak-tools -f packaging/Dockerfile.flatpak-tools .
mkdir -p build/flatpak-cache build/flatpak-work dist/flatpak-experimental
docker run --rm --privileged \
  -v "$PWD:/source:ro" \
  -v "$PWD/build/flatpak-cache:/root/.local/share/flatpak" \
  -v "$PWD/build/flatpak-work:/work" \
  -v "$PWD/dist/flatpak-experimental:/output" \
  lse-flatpak-tools sh -c '
    flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo &&
    flatpak install --user -y --noninteractive flathub org.freedesktop.Platform//26.08 &&
    python3 /source/scripts/flatpak.py build --stage /source/build/flatpak-stage/app --work-dir /work --output /output
  '
```

A imagem usa o proxy D-Bus da distribuição e inclui o loader SVG exigido para exportar o ícone. Estes comandos exportam o bundle, sem instalar o aplicativo no host. Os testes gráficos em container usam D-Bus e Xvfb próprios; não substituem testes na sua sessão KDE/GNOME.

## Exemplo didático de manifesto — somente local

[flatpak-local.json](examples/flatpak-local.json) foi gerado com assistência de IA **para aprender e testar localmente**. Não é material de submissão ao Flathub e não deve ser usado como base do manifesto enviado à loja. Ele apenas copia um staging já compilado, por isso usa a Platform também como ambiente de cópia, sem toolchain. Não é um exemplo de compilação Cargo no SDK.

Depois do staging padrão acima, você pode testar esse caminho alternativo:

```sh
flatpak-builder --user --force-clean --repo=build/flatpak-example-repo build/flatpak-example docs/examples/flatpak-local.json
```

| Campo | Significado neste exemplo |
| --- | --- |
| `app-id` | Identidade constante; coincide com o desktop, o ícone e o AppStream |
| `runtime` e `runtime-version` | Bibliotecas fornecidas ao executar; branch da plataforma |
| `sdk` | Ambiente de build; aqui só copia arquivos já compilados; uma compilação Rust real usa um SDK com toolchain |
| `command` | Executável instalado em `/app/bin` |
| `branch` | Canal local `experimental`, separado de um eventual `stable` |
| `finish-args` | Acessos concedidos ao executar o aplicativo |
| `modules` | Etapas que colocam os arquivos em `/app` |
| `sources` | De onde vêm os arquivos; aqui um diretório local de staging |

Para aprender o formato a partir de material oficial, consulte o [primeiro build](https://docs.flatpak.org/en/latest/first-build.html), os [manifestos](https://docs.flatpak.org/en/latest/manifests.html) e o [gerador Cargo oficial](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo). Não envie este exemplo de binário local como se fosse um build offline de fontes.

## Caminho para a loja

O projeto ainda está em alpha. As [exigências do Flathub](https://docs.flathub.org/docs/for-app-authors/requirements) incluem software estável, histórico significativo e suporte upstream para gerenciadores de arquivos. A política também exige divulgação do uso de IA e proíbe conteúdo assistido por IA no manifesto e nas interações automatizadas de submissão. A autoria humana precisa ser independente; reescrever ou copiar o exemplo acima não remove essa condição.

1. Consolidar o protótipo: manter os conversores incluídos, definir Lixeira e integrações, executar a matriz de testes da tabela e resolver limitações de uso cotidiano.
2. Definir e manter a versão estável suportada, com releases rastreáveis, responsável por atualizações e revisão dos componentes/licenças incluídos.
3. Revisar manualmente AppStream, descrição, OARS e inglês/português. Publicar screenshots reais com dados sintéticos e URLs permanentes; os screenshots e o histórico de release da loja ainda estão pendentes.
4. Você escreve o manifesto de submissão a partir da documentação oficial, com fontes fixadas e builds offline no SDK; resolve as dependências Cargo com a ferramenta oficial e revisa o resultado. Não use o manifesto didático como base.
5. Testar o build, instalar e rodar o linter com `org.flatpak.Builder`, seguindo a [submissão oficial](https://docs.flathub.org/docs/for-app-authors/submission). Justificar permissões e resolver ou obter análise das exceções necessárias.
6. Você faz o fork e a submissão humana à branch `new-pr`, preenche a divulgação de IA com base no histórico real e responde aos revisores com suas próprias palavras. Não há PR, texto de submissão ou aprovação preparados automaticamente aqui.
7. Após aceitação, verificar a propriedade da conta/repositório com `janderpt`, publicar pelo processo da loja e manter o runtime e os módulos atualizados. Aceitação não é garantida.

Políticas consultadas em 2026-11-12. Esta é documentação técnica do projeto, não um texto de PR ou uma declaração de revisão humana concluída.

## English summary

This is an **experimental Flatpak** distributed as a separate GitHub release asset, not a Flathub release. Install the bundle with `flatpak install --user FILE.flatpak`, run `flatpak run --branch=experimental io.github.janderpt.linux-system-explorer`, and uninstall that branch with `flatpak uninstall --user io.github.janderpt.linux-system-explorer//experimental`. The Freedesktop 26.08 runtime is downloaded separately. The recommended flatpak-previews-dbus6 bundle uses zbus 6.2.1 with a verified local sequential-authentication backport, supporting the Ubuntu 24.04 distribution proxy 0.1.5 without manually replacing it. Both proxies 0.1.5 and 0.1.9 were tested. Earlier alpha.4 variants remain available with their original requirements and checksums. Use distribution-maintained Flatpak/D-Bus packages and normal system updates; the runtime does not update the host proxy. See DBUS-COMPATIBILITY.md for reproducible tests. Preferences are sandbox-specific and neither installation nor removal changes the default file manager. The unsigned local bundle has no automatic repository updates. To upgrade the previous alpha.4 converter-less build or alpha.2 prototype, close its windows and install the new bundle on the same experimental branch without uninstalling or deleting data. The automated disposable-container smoke test covers fresh installation, same-ref upgrade, exact preference preservation, Xvfb startup and removal; this does not certify a real desktop session.

Images/text, GStreamer media and libarchive use embedded/runtime components. The current development build also bundles Poppler, LibreOffice and WeasyPrint, with pinned inputs, preserved notices and a separate corresponding-source archive. The original converter-less alpha.4 bundle remains unchanged as a historical asset; use the new **flatpak-previews-dbus6** variant and **FLATPAK-DBUS6-SHA256SUMS**. See FLATPAK-PREVIEW.md for the offline SDK build and sandbox renderer tests. Opening files uses the desktop portal; terminal commands run inside the sandbox. New remote connections, local Samba management, UDisks2 volume discovery/mount/unmount/eject and FileManager1 host registration are not enabled. Trash parity, mounted paths, DND and both desktop/window-system combinations require real-session validation. The build report records actual runtime capabilities and permissions. Keep the native package for full existing integrations.

The local JSON example is AI-assisted educational material, repackaging an existing staging directory, and is explicitly unsuitable for a Flathub submission or its starting point. The eventual store manifest and submission/review interactions must be independently human-authored. A stable, maintained release, a source/offline SDK build, complete converters, portal/desktop tests, metadata/screenshots, license review and manual store review are still required. See the linked official documentation rather than copying this local example into a submission.
