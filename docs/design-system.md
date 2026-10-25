# BindForge Glass

O tema adapta as referências visuais fornecidas pelo usuário em 24 de outubro
de 2026: vidro escuro, índigo e violeta, superfícies com transparência e reflexos
discretos nas bordas. Os componentes são implementados em CSS e SVG locais, sem
fontes remotas, imagens de fundo ou bibliotecas adicionais.

## Tokens

Os tokens ficam em `apps/desktop/src/style.css`. Componentes e ilustrações devem
usar esses nomes para evitar variações de paleta entre telas.

| Token | Valor inicial | Uso |
| --- | --- | --- |
| `--canvas` | `#0b0a15` | Fundo do aplicativo |
| `--surface` | `#181625` | Superfície opaca de apoio |
| `--surface-soft` | `#201b358c` | Cartões e grupos de campos |
| `--surface-inset` | `#100e1c` | Campos e áreas internas |
| `--text` | `#f2effb` | Texto principal |
| `--text-muted` | `#b7b0cb` | Texto de apoio |
| `--accent` | `#a679f5` | Realces e mapeamentos |
| `--accent-light` | `#d2baff` | Links, foco e seleção |
| `--success` | `#a1e8ce` | Captura ativa e conexão |
| `--warning` | `#f1ce91` | Erros que precisam de atenção |
| `--danger` | `#ffc1d2` | Ações destrutivas |
| `--radius` | `18px` | Cartões e painéis |

`--glass`, `--glass-highlight`, `--glow` e os tokens de borda compõem o efeito de
vidro. Os tokens `--device-*` mantêm mouse e teclado na mesma paleta. Campos,
teclas e estados usam fundos suficientemente definidos para preservar a leitura.

## Componentes e hierarquia

- Uma moldura translúcida envolve a aplicação. A barra superior reúne a marca
  e o estado do serviço; o estado é um atalho para Configurações.
- A navegação lateral abre Dispositivos, Perfis e Configurações.
  Em janelas estreitas, ela se reorganiza acima do conteúdo.
- Dispositivos mostra os cartões com Configurar, Ativar/Pausar e Acesso. O perfil
  em uso fica no rodapé. Adicionar dispositivo abre o cadastro; gerenciamento de
  interfaces e ações gerais ficam nas opções da mesma área.
- Serviço, integração KDE, histórico, teste de seleção e informações de versão
  ficam em Configurações.
- Botões principais usam o gradiente violeta; ações secundárias usam superfícies
  discretas. Exclusões mantêm a cor de perigo e confirmações existentes.
- Seleção na navegação tem borda, fundo e peso de texto. Estados são nomeados;
  a cor nunca é o único indicador.
- Mouse e teclado abrem em uma página com desenho à esquerda e edição de ações
  à direita. O perfil em edição fica separado do perfil em uso. Salvar mantém a
  página aberta. Rascunhos são protegidos ao voltar, trocar perfil, área ou fechar
  a janela. Modais ficam para cadastro, acesso e confirmações.

## Renderização e acessibilidade

- O blur de 18px fica na moldura principal. Os cartões internos usam gradientes
  e transparência sem repetir `backdrop-filter` em cada elemento.
- O backdrop de um modal usa o blur já existente de 3px; o modal mantém fundo
  definido. Não há animações contínuas, timers ou novos listeners de movimento.
- Sem suporte a backdrop-filter, a moldura usa fundo opaco. A preferência
  `prefers-reduced-transparency` remove o blur e reforça as superfícies.
- `prefers-reduced-motion` desativa transições; `forced-colors` preserva bordas
  e indicação da aba selecionada. O foco permanece visível e a navegação de abas
  aceita setas, Home e End.
- Os efeitos são limitados à renderização da UI. O leitor evdev/uinput não foi
  alterado nesta etapa. O custo de composição gráfica não foi medido.

## Prévia local

Com Vite em execução, `tests/glass-browser.html` exercita os componentes reais
com perfis e dispositivos sintéticos e armazenamento em memória. A fixture não
ativa dispositivos nem gerencia o serviço real. Os botões de acesso nativo não
devem ser usados como teste de autorização nessa fixture.

O build web e os testes do frontend validam tipos e comportamento de apoio;
A revisão de 24 de outubro incluiu inspeção visual em 960 × 800, salvamento de
mouse e teclado sem ativação, proteção ao trocar perfil/área e restauração de foco.
Os 30 testes do frontend passaram, incluindo validação de atalhos incompletos e
preservação de ações de outros dispositivos. A captura nativa não foi ativada.
O RPM desta revisão é `BindForge-0.1.0-3.x86_64.rpm`.

## Dropdowns

`AppSelect` usa um menu do próprio aplicativo com Radix Select, estilizado com os
tokens de superfície, borda, destaque e tipografia. Todos os campos de seleção da
interface usam o mesmo componente, incluindo os formulários em modais nativos.
O popup é inserido no `dialog` mais próximo para respeitar a camada e o foco.

As opções podem ser agrupadas e desabilitadas. A navegação usa setas, Home/End,
Enter, Escape e busca por digitação. Um campo nativo oculto preserva a validação de
campos obrigatórios e os valores enviados pelo formulário, inclusive valores vazios.
O estado de `fieldset disabled` também é respeitado pelo menu.

A fixture `apps/desktop/tests/select-browser.html` permite conferir esses casos
sem modificar perfis ou acessar o daemon.
