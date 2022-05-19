import "./claytip.d.ts"

interface CaptchaChallenge {
    uuid: string,
    response: string,
}

export async function getChallenge(claytip: Claytip): CaptchaChallenge {


    let result = await claytip.executeQuery(`
        query {

        } 
    `, {
        "properResponse": ""
    });

    return {
        uuid: result.uuid,
        question: question
    }
}

interface CaptchaValidatorContext {
    uuid: string,
    response: string
}

export async function verifyCaptcha(claytip: Claytip, context: CaptchaValidatorContext) {

}