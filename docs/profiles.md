# Configuração de perfis

A biblioteca `remap-config` é compartilhada pelo desktop Tauri e pelo daemon KDE.
O formato JSON usa `version: 1`, `revision` inteiro de 32 bits, `fallback_id` e uma
lista `profiles`. Uma versão desconhecida é rejeitada, sem migração ou sobrescrita.
Os exemplos iniciais incluem Padrão, Firefox e Konsole, para preservar a demonstração
anterior. Eles não são gravados no disco até o primeiro salvamento pelo usuário.

```json
{
  "version": 1,
  "revision": 1,
  "fallback_id": "default",
  "profiles": [
    {
      "id": "default", "name": "Padrão", "enabled": true,
      "priority": 0, "rules": [], "bindings": []
    },
    {
      "id": "personal-browser", "name": "Navegação", "enabled": true,
      "priority": 10,
      "rules": [{ "app_id": "firefox", "executable": null, "title_contains": null }],
      "bindings": [{
        "device_id": null,
        "input": { "MouseButton": 8 },
        "action": { "KeyChord": ["Alt", "Left"] }
      }]
    }
  ]
}
```

As ações são simbólicas e continuam simuladas. O formulário de metadados preserva ou copia ações; a aba Início permite editar
os cinco controles genéricos do mouse. Copiar cria um mapa independente; não há
herança de alterações posteriores. O ID do novo perfil é gerado por UUID na UI.
O botão **Detectar pela janela** abre um painel no formulário e arma uma captura
no daemon. Após **Captura pronta**, selecione outro aplicativo pelo mouse ou Alt+Tab.
O primeiro ID diferente do aplicativo em foco no início é incluído na lista, sem
duplicar IDs existentes. A ausência de foco e repetições do aplicativo inicial são
ignoradas. A captura expira em 30 segundos e pode ser cancelada ou repetida.
Requer o daemon --kde com sua ponte temporária conectada ao KWin. O ID fica visível para conferência; salvar
continua sendo uma ação explícita do usuário. A identificação usa os mesmos IDs
exatos usados pela seleção de perfis (desktopFileName, com fallback resourceClass).

A seleção usa as regras do core: maior prioridade, maior especificidade da regra
que corresponde e menor ID lexical. O formulário informa sobreposições e oferece
prioridade nas opções avançadas. Regras com executável ou título são preservadas
integralmente ao editar nome, prioridade e ativação; suas associações ficam bloqueadas.

A edição é aberta pelo botão de lápis no card. **Excluir perfil** fica no rodapé
do modal de edição e abre uma confirmação. Cancelar a confirmação preserva o
rascunho; confirmar remove o perfil e fecha a edição. O perfil Padrão não oferece
exclusão.

## Ícones dos perfis

Cada perfil aparece em um card e pode ter uma imagem própria. O envio manual aceita
PNG, JPG, WebP e ICO de até 5 MB; a imagem é convertida para PNG ao salvar. Em **Editar perfil**,
use **Usar ícone do aplicativo** para procurar os ícones dos IDs associados. Um
único resultado é selecionado diretamente; com vários resultados, escolha o
aplicativo que representará o perfil. Se nada for encontrado, o ícone atual é
preservado e o envio manual de imagem continua disponível.

No Linux, a busca usa as entradas `.desktop` e o tema de ícones do sistema,
incluindo correspondência por `StartupWMClass`. IDs `steam:ID` usam os ícones
instalados dos jogos e, quando disponíveis, imagens do cache local da Steam,
incluindo a instalação Flatpak. A busca não baixa imagens nem executa aplicativos.

**Detectar pela janela** também procura o ícone do aplicativo detectado quando o
perfil ainda não tem imagem. Um ícone escolhido anteriormente não é substituído.
A imagem só é gravada ao salvar o perfil.

O campo opcional `icon` armazena um PNG em data URL, com até 96 px no maior lado
para ícones locais e até 64 KiB de conteúdo base64. Configurações antigas, sem esse
campo, continuam válidas e usam o símbolo padrão. **Usar ícone padrão** remove a
imagem. Como a imagem fica no perfil, ela continua visível se o aplicativo original
for desinstalado ou seu tema mudar.

A fixture `apps/desktop/tests/profile-icons-browser.html` permite conferir seleção
entre aplicativos, ausência de ícone e captura simulada sem alterar configurações
reais. Os testes nativos de `application_icons` cobrem correspondência e PNG; a
verificação opcional de ícones instalados requer uma sessão gráfica:

```bash
BINDFORGE_ICON_TEST_IDS='org.kde.konsole,steam:1808500' \
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml \
  application_icons::tests::installed_application_icons -- --ignored --test-threads=1
```

Use IDs de aplicativos instalados na sua máquina nesse comando.

## Armazenamento

- Linux: `$XDG_CONFIG_HOME/bindforge/profiles.json`, com fallback para
  `$HOME/.config/bindforge/profiles.json`. XDG relativo é ignorado.
- Caminhos convencionais de configuração por usuário estão previstos em Windows
  e macOS; a integração do daemon permanece exclusiva do KDE Linux e estes alvos
  não foram validados.
- A primeira execução migra os perfis da pasta anterior para `bindforge`,
  preservando o arquivo original e a revisão. Um destino existente nunca é sobrescrito.
- `BINDFORGE_CONFIG` substitui o caminho nos dois processos, para testes ou
  diagnóstico. Defina o mesmo valor para UI e daemon.

O salvamento valida antes de escrever, usa um lock exclusivo, verifica a revisão
lida pelo editor, sincroniza um arquivo temporário e o renomeia sobre o destino.
Arquivos novos têm modo 0600 no Unix. A UI não aceita caminhos de arquivo arbitrários.
Uma janela com revisão antiga recebe erro e deve cancelar a edição, recarregar a
lista e tentar novamente. Arquivos inválidos ou de versão futura não são sobrescritos.
Não há backup automático nem mesclagem de edições externas.

Se o processo for encerrado durante uma gravação, o arquivo de lock ou o temporário
pode permanecer. O erro identifica o lock (`profiles.json.lock`); após confirmar que
nenhuma gravação está em andamento, remova apenas esse lock e eventual temporário
`profiles.json.<pid>.tmp` para repetir. A revisão é controle de concorrência entre
instâncias do app; um editor externo que não respeita lock/revisão fica fora dessa
garantia. Apagar o arquivo faz a leitura voltar aos exemplos iniciais.

Limites: 1 MiB por arquivo, 256 perfis, 256 regras e 512 ações por perfil. IDs e
nomes têm até 128 bytes; predicados e IDs de dispositivos, até 4096 bytes. Controles,
nomes vazios, IDs duplicados, fallback desativado/com regras, regras vazias e ações
malformadas são rejeitados. Perfis específicos precisam de pelo menos uma regra.
Essas validações não substituem a validação de capacidades de um futuro backend.

## Recarregamento e validação

O modo `--kde` lê o arquivo na inicialização e após cada espera/evento do monitor,
com espera ociosa de 500 ms mais o tempo de consulta D-Bus. O daemon reavalia o
último foco autenticado e publica o novo estado, sem exigir mudança de janela.
Mudanças de dono do KWin são processadas antes da reavaliação. Configuração inválida
em execução mantém a anterior; inválida na inicialização encerra com erro. A prévia
nativa lê o arquivo atual; a demo finita e `--stdin` permanecem fixtures fixas.
A mensagem de salvamento da UI confirma a gravação, sem confirmação transacional
de aplicação pelo daemon. O painel do daemon mostra a seleção consumida.

Testes cobrem roundtrip, revisão obsoleta, preservação após falha, versões e
validação, reaplicação do mesmo ID com ações novas e rejeição de substituição pelo
backend. Em D-Bus privado, o daemon real recebe um perfil salvo sem evento de foco,
publica a nova seleção, mantém a política após corrupção e recupera após restauração.
O frontend cobre cópia independente, deduplicação, conflitos, fallback e preservação
de regras avançadas. A fixture `tests/profiles-browser.html` permite conferir criação
e edição com armazenamento somente em memória, sem tocar nos perfis do usuário.


## Captura temporária de aplicativo

A interface D-Bus `org.bindforge.Daemon.Capture1` oferece `BeginCapture → t`,
`ReadCapture(t) → (s, s)` e `CancelCapture(t)`. O resultado é `waiting` com ID vazio
ou `captured` com o ID identificado. Existe uma única captura ativa no daemon,
associada ao remetente único que a iniciou e ao dono atual de KWin. Outro cliente
não pode ler ou cancelar essa captura; uma tentativa simultânea recebe erro.
O pedido tem prazo de 30 segundos. O token e o resultado não são persistidos.

O receptor autentica eventos como antes, aceitando apenas o dono de `org.kde.KWin`.
Uma sequência de eventos impede usar eventos anteriores ao armamento; a captura
ignora ausência de foco e repetições do aplicativo inicial. O primeiro resultado
não é substituído ao retornar à janela de configuração. Reinício de KWin ou do
daemon interrompe a tentativa. Não há coleta de títulos, imagens ou entrada física.
O serviço de estado continua sem expor os IDs de aplicativos.

Os comandos Tauri `start_window_capture`/`stop_window_capture` usam conexão própria,
fixada ao dono do daemon, com Channel para resultado/erro. A UI só anuncia prontidão
depois que o daemon arma a captura. Durante essa captura explícita, o cliente consulta
seu resultado a cada 100 ms; o painel de estado continua usando sinais, sem polling.
Cancelar, fechar o painel/formulário e recarregar/fechar a janela cancelam a tentativa.
Respostas atrasadas não alteram o formulário. A perda abrupta do cliente pode deixar
a vaga ocupada até expirar o prazo. Um daemon antigo pede reinício.

A fixture visual inclui botões para simular foco e timeout, sem capturar janelas reais
nem gravar os perfis do usuário. Os testes D-Bus usam um bus privado e verificam
captura real pelo protocolo, autenticação, cancelamento, rearmamento e desconexão.


## Steam e Gamescope

A associação `steam:<AppId>` identifica o jogo quando o daemon consegue ler o
`SteamAppId` da instância Gamescope correspondente à janela ativa, ou o marcador
explícito de lançamento `reaper SteamLaunch AppId=<id> --` em seu comando filho,
incluindo o invólucro Steam conhecido `steam-launch-wrapper --`. Por exemplo,
`steam:1808500` e `steam:570` selecionam perfis distintos mesmo que ambos os
executáveis externos sejam Gamescope. A normalização é feita antes da captura e
da seleção de perfis, evitando salvar uma identidade que o daemon não reconheça.
Não se trata de um nome de processo ou de um PID salvo no perfil.

A resolução atual é para uma instância Gamescope iniciada por jogo. Não identifica
qual cliente está em foco dentro de uma instância Gamescope compartilhada. Se a
variável Steam não estiver disponível ou o PID for inacessível, a captura informa
que não conseguiu individualizar o jogo e preserva o formulário. Uma associação
manual `gamescope` é ampla e só corresponde a instâncias sem identidade resolvida;
ela não é convertida automaticamente para um jogo. Recapture e remova a associação
genérica dos perfis antigos para torná-los específicos. A ponte KWin atual é incorporada ao daemon e transmite o PID da janela
automaticamente ao iniciar a versão atual.


Mesmo quando o KWin deixa os IDs da janela Gamescope vazios, o PID autenticado pode
fornecer a identidade. Se o processo for reconhecido como Gamescope, mas nenhum
jogo puder ser identificado, a captura entrega o resultado genérico para a UI
explicar a limitação imediatamente. Isso evita o timeout causado por tratar essa
janela como ausência de foco. O processo real do ARC Raiders foi identificado como
`steam:1808500` durante o diagnóstico; o daemon precisa da versão corrigida.


## Modais da interface

Criar/editar perfil, detectar uma janela e confirmar exclusão usam o componente
compartilhado Modal com dialog nativo e portal. O navegador mantém o fundo inerte,
o foco no diálogo superior e o rolamento da página bloqueado enquanto houver um
modal aberto. O conteúdo longo rola dentro do editor; o cabeçalho permanece visível.

A captura abre um segundo modal sobre o editor. Fechar esse modal mantém o nome,
os aplicativos e as demais opções preenchidas. Escape fecha somente o modal
superior; também existem os botões Fechar/Cancelar. Cancelar o editor descarta o
rascunho como antes. Durante uma gravação, o fechamento do modal correspondente
fica bloqueado até terminar, evitando perder a exibição do resultado da operação.
Erros de gravação e validação aparecem dentro do modal que iniciou a operação.

Ao fechar, o foco volta ao botão que abriu o modal, se ele ainda existir. A exclusão
inicia com o foco em Cancelar. Nenhuma exclusão é executada por Escape ou fechamento.
A fixture de perfis usa um seletor de resultados sintéticos e uma captura simulada
com retorno temporizado, para exercitar os modais sem depender de janelas reais.


## Edição visual de ações na aba Início

A seleção do dropdown Editar perfil não altera o perfil ativo no daemon. O modal
Mouse guarda um snapshot do perfil/revisão ao abrir. As alterações ficam locais
até Salvar alterações; aplicar um controle modifica apenas esse rascunho. Fechar
com diferenças pede confirmação. O salvamento altera apenas bindings do perfil
selecionado e usa a revisão inicial: uma edição externa ou metadados mais recentes
não são sobrescritos. Erros mantêm o rascunho no modal.

As ações suportadas pelo editor são KeyChord, MouseButton e Disabled, no formato
JSON existente. Uma tecla isolada é um KeyChord de um item. O editor valida teclas
não vazias, até 16 itens, sem duplicatas ou controles embutidos. Isso não garante
suporte a um código evdev/HID futuro. Símbolos e números dos controles continuam
sendo de configuração/simulação.

O escopo null representa todos os dispositivos compatíveis. Um device_id específico
tem prioridade para o mesmo InputControl, tanto em Profile::binding_for no core
quanto na prévia do editor. O seletor inclui mouses/interfaces compostas e preserva
escopos salvos que estejam indisponíveis. IDs atuais não garantem reencontro após
reinício/troca de porta. Dispositivos salvos indisponíveis não são reassociados a
outro hardware por suposição.

Renomear/copiar/excluir perfis segue na aba Perfis. Ambas as telas recebem a
configuração após salvamentos, evitando revisões locais divergentes. A prévia
por app ID fica em Perfis; atividade do daemon e histórico ficam em Início.
A tela de dispositivos mantém sua atualização manual.

### Editor de teclado

O cartão do teclado em Início abre um editor do perfil escolhido, com snapshot de
revisão e as mesmas proteções de salvamento e descarte do mouse. O desenho segue
o layout cadastrado: ANSI (104 teclas), ISO (105) ou ABNT2 (107), com Enter, Shift,
Ç, tecla extra de barra e teclado numérico correspondentes. São modelos completos
de referência, sem detecção de formato compacto ou mudança do layout do sistema.
O desenho ajusta sua largura e mantém rolagem horizontal em telas estreitas.

Clique em uma tecla, ou use as setas e Enter, para configurar a ação no painel.
O destino pode ser outra tecla, um atalho com modificadores, botão do mouse ou
Disabled. As mudanças entram diretamente no rascunho e só são persistidas em
Salvar alterações. Voltar ao comportamento original remove a ação específica;
uma ação geral existente continua aparecendo como herdada. A lista de mapeamentos
permite acessar inclusive ações existentes para teclas ausentes no desenho.

Novas entradas Key e destinos KeyChord usam nomes físicos KEY_* do Linux,
independentes da legenda: no ABNT2, Ç corresponde a KEY_SEMICOLON. A prévia também
reconhece nomes simbólicos anteriores, como CapsLock e A, preservando as demais
ações de mouse, interfaces e perfis. O backend de teclado também normaliza
as entradas antes de compilar os mapas; colisões entre aliases no mesmo escopo são
recusadas. Salvar não ativa a captura física: use Remapeamento do teclado na Home.
Com o teclado já ativo, mudanças no perfil ativo são aplicadas pelo daemon.

A variável de ambiente anterior continua aceita como fallback de compatibilidade.
